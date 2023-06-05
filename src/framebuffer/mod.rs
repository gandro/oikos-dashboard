use std::borrow::Cow;
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::mem::MaybeUninit;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{io, thread};

use bitflags::bitflags;
use log::debug;
use memmap2::{MmapMut, MmapOptions};
use thiserror::Error;
use tiny_skia::{ColorU8, Pixmap};

use self::sys::{fb_fix_screeninfo, fb_var_screeninfo};

mod sys;

const EINK_REFRESH_WAIT: Duration = Duration::from_millis(1000);

#[derive(Debug, Error)]
pub enum Error {
    #[error("OS error")]
    OsError(#[from] nix::Error),
    #[error("I/O error")]
    IoError(#[from] io::Error),
    #[error("Unsupported pixel depth")]
    UnsupportedPixelDepth,
    #[error("Unsupported pixel format")]
    UnsupportedPixelFormat,
}

type PixelDepth = u32;

#[derive(Copy, Clone, Debug)]
enum PixelFormat {
    RGBA8888,
    BGRA8888,
    RGB888,
    BGR888,
    RGB565,
    BGR565,
    Grayscale8,
    Grayscale16,
    Grayscale32,
}

// Computes the brightness of a color using Rec. 709 coefficients
fn luma(c: ColorU8) -> f64 {
    let red = c.red() as f64 / 255.;
    let green = c.green() as f64 / 255.;
    let blue = c.blue() as f64 / 255.;

    (0.2126 * red + 0.7152 * green + 0.0722 * blue).clamp(0., 1.)
}

fn invert(c: ColorU8) -> ColorU8 {
    ColorU8::from_rgba(
        u8::MAX - c.red(),
        u8::MAX - c.green(),
        u8::MAX - c.blue(),
        u8::MAX - c.alpha(),
    )
}

// Writes a variable length integer `src` (in big endian encoding) to the slice
// `dst` in native endian order.
fn write_varint<const N: usize>(dst: &mut [u8], mut src: [u8; N]) {
    #[cfg(target_endian = "little")]
    {
        src.reverse()
    }
    dst[..N].copy_from_slice(&src)
}

impl PixelFormat {
    fn draw(&self, c: ColorU8, buf: &mut [u8]) {
        match self {
            PixelFormat::RGBA8888 => {
                write_varint(buf, [c.red(), c.green(), c.blue(), c.alpha()]);
            }
            PixelFormat::BGRA8888 => {
                write_varint(buf, [c.blue(), c.green(), c.red(), c.alpha()]);
            }
            PixelFormat::RGB888 => {
                write_varint(buf, [c.red(), c.green(), c.blue()]);
            }
            PixelFormat::BGR888 => {
                write_varint(buf, [c.blue(), c.green(), c.red()]);
            }
            PixelFormat::RGB565 => {
                write_varint(
                    buf,
                    [
                        (c.red() & 0b1111_1000) | (c.green() & 0b0000_0111 >> 5),
                        (c.green() & 0b1110_0000) | (c.blue() & 0b0001_1111 >> 3),
                    ],
                );
            }
            PixelFormat::BGR565 => {
                write_varint(
                    buf,
                    [
                        (c.blue() & 0b1111_1000) | (c.green() & 0b0000_0111 >> 5),
                        (c.green() & 0b1110_0000) | (c.red() & 0b0001_1111 >> 3),
                    ],
                );
            }
            PixelFormat::Grayscale8 => {
                let v = (luma(c) * u8::MAX as f64).round() as u8;
                write_varint(buf, [v]);
            }
            PixelFormat::Grayscale16 => {
                let v = (luma(c) * u16::MAX as f64).round() as u16;
                write_varint(buf, v.to_be_bytes());
            }
            PixelFormat::Grayscale32 => {
                let v = (luma(c) * u32::MAX as f64).round() as u32;
                write_varint(buf, v.to_be_bytes());
            }
        }
    }
}

fn pixel_format(var_screeninfo: fb_var_screeninfo) -> Result<(PixelFormat, PixelDepth), Error> {
    if var_screeninfo.grayscale == 1 {
        return match var_screeninfo.bits_per_pixel {
            bpp @ 32 => Ok((PixelFormat::Grayscale32, bpp)),
            bpp @ 16 => Ok((PixelFormat::Grayscale16, bpp)),
            bpp @ 8 => Ok((PixelFormat::Grayscale8, bpp)),
            _ => Err(Error::UnsupportedPixelFormat),
        };
    }

    let r = var_screeninfo.red;
    let g = var_screeninfo.green;
    let b = var_screeninfo.blue;
    let a = var_screeninfo.transp;

    let pixel_format = (
        (r.offset, r.length, r.msb_right),
        (g.offset, g.length, g.msb_right),
        (b.offset, b.length, b.msb_right),
        (a.offset, a.length, a.msb_right),
    );

    const NONE: (u32, u32, u32) = (0, 0, 0);

    const BIT8_0: (u32, u32, u32) = (0, 8, 0);
    const BIT8_1: (u32, u32, u32) = (8, 8, 0);
    const BIT8_2: (u32, u32, u32) = (16, 8, 0);
    const BIT8_3: (u32, u32, u32) = (24, 8, 0);

    const BIT5_0: (u32, u32, u32) = (0, 5, 0);
    const BIT6_1: (u32, u32, u32) = (5, 6, 0);
    const BIT5_2: (u32, u32, u32) = (11, 5, 0);

    match var_screeninfo.bits_per_pixel {
        bpp @ (32 | 24) => match pixel_format {
            (BIT8_0, BIT8_1, BIT8_2, BIT8_3) => Ok((PixelFormat::RGBA8888, bpp)),
            (BIT8_2, BIT8_1, BIT8_0, BIT8_3) => Ok((PixelFormat::BGRA8888, bpp)),
            (BIT8_0, BIT8_1, BIT8_2, NONE) => Ok((PixelFormat::RGB888, bpp)),
            (BIT8_2, BIT8_1, BIT8_0, NONE) => Ok((PixelFormat::BGR888, bpp)),
            _ => Err(Error::UnsupportedPixelFormat),
        },
        bpp @ 16 => match pixel_format {
            (BIT5_0, BIT6_1, BIT5_2, NONE) => Ok((PixelFormat::RGB565, bpp)),
            (BIT5_2, BIT6_1, BIT5_0, NONE) => Ok((PixelFormat::BGR565, bpp)),
            _ => Err(Error::UnsupportedPixelFormat),
        },
        _ => Err(Error::UnsupportedPixelDepth),
    }
}

fn device_id<'a>(fix_screeninfo: &'a fb_fix_screeninfo) -> Cow<'a, str> {
    unsafe { CStr::from_ptr(fix_screeninfo.id.as_ptr()).to_string_lossy() }
}

bitflags! {
    #[derive(Copy, Clone, Debug)]
    struct DeviceFeatures: u32 {
        const INVERTED_COLOR = 0b0000_0001;
        const KINDLE_LEGACY_EINK_REFRESH = 0b0000_0010;
    }
}

impl DeviceFeatures {
    fn from_id(device_id: &str) -> Self {
        match device_id {
            "eink_fb" => DeviceFeatures::KINDLE_LEGACY_EINK_REFRESH | DeviceFeatures::INVERTED_COLOR,
            _ => DeviceFeatures::empty(),
        }
    }
}

#[derive(Default, Debug)]
pub struct Builder {
    device: PathBuf,
    eink_refresh_rate: u32,
}

impl Builder {
    pub fn with_device(device: PathBuf) -> Self {
        Builder {
            device: device,
            eink_refresh_rate: 0,
        }
    }

    pub fn eink_refresh_rate(mut self, rate: u32) -> Self {
        self.eink_refresh_rate = rate;
        self
    }

    pub fn open(self) -> Result<Framebuffer, Error> {
        let mut fb = Framebuffer::open(self.device)?;
        fb.eink_refresh_rate = self.eink_refresh_rate;
        Ok(fb)
    }
}

#[derive(Debug)]
pub struct Framebuffer {
    dev: File,
    buf: MmapMut,
    xres: u32,
    yres: u32,
    stride: u32,
    bits_per_pixel: PixelDepth,
    pixel_format: PixelFormat,
    features: DeviceFeatures,
    eink_refresh_rate: u32,
    draw_count: u64,
}

impl Framebuffer {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let dev = OpenOptions::new().read(true).write(true).open(&path)?;

        let fd = dev.as_raw_fd();
        let fix_screeninfo = unsafe {
            let mut fix_screeninfo = MaybeUninit::<sys::fb_fix_screeninfo>::zeroed();
            sys::fb_get_fix_screeninfo(fd, fix_screeninfo.as_mut_ptr())?;
            fix_screeninfo.assume_init()
        };

        let var_screeninfo = unsafe {
            let mut var_screeninfo = MaybeUninit::<sys::fb_var_screeninfo>::zeroed();
            sys::fb_get_var_screeninfo(fd, var_screeninfo.as_mut_ptr())?;
            var_screeninfo.assume_init()
        };

        let xres = var_screeninfo.xres;
        let yres = var_screeninfo.yres;
        let stride = fix_screeninfo.line_length;
        let framelen = stride
            .checked_mul(yres)
            .and_then(|i| i.try_into().ok())
            .expect("framelen integer overflow");
        let buf = unsafe { MmapOptions::new().len(framelen).map_mut(&dev)? };

        let (pixel_format, bits_per_pixel) = pixel_format(var_screeninfo)?;
        let id = device_id(&fix_screeninfo);
        let features = DeviceFeatures::from_id(&id);

        debug!(
            "Mapped framebuffer device {:?} as {:?}. Resolution: {}x{}@{}bpp ({:?}, {:?})",
            path.as_ref(),
            id,
            xres,
            yres,
            bits_per_pixel,
            pixel_format,
            features
        );

        let eink_refresh_rate = 0;
        let draw_count = 0;
        Ok(Framebuffer {
            dev: dev,
            buf,
            xres,
            yres,
            stride,
            pixel_format,
            bits_per_pixel,
            features,
            eink_refresh_rate,
            draw_count,
        })
    }

    fn needs_eink_refresh(&self) -> bool {
        if self.draw_count == 0 {
            return true;
        }

        if self.eink_refresh_rate == 0 {
            return false;
        };

        (self.draw_count % self.eink_refresh_rate as u64) == 0
    }

    pub fn screen_size(&self) -> (u32, u32) {
        (self.xres, self.yres)
    }

    pub fn draw(&mut self, pixmap: Pixmap) -> Result<(), Error> {
        let pixel_len = self.bits_per_pixel / 8;
        for y in 0..pixmap.height().min(self.yres) {
            for x in 0..pixmap.width().min(self.xres) {
                let offset = (y * self.stride) + (x * pixel_len);

                let mut c = pixmap.pixel(x, y).expect("invalid pixel").demultiply();
                if self.features.contains(DeviceFeatures::INVERTED_COLOR) {
                    c = invert(c)
                }

                self.pixel_format.draw(c, &mut self.buf[offset as usize..]);
            }
        }

        if self.features.contains(DeviceFeatures::KINDLE_LEGACY_EINK_REFRESH) {
            let fx = match self.needs_eink_refresh() {
                true => sys::fx_type::fx_update_full,
                false => sys::fx_type::fx_update_partial,
            };

            unsafe {
                sys::fbio_eink_update_display(self.dev.as_raw_fd(), fx as std::ffi::c_int)?;
            }

            thread::sleep(EINK_REFRESH_WAIT);
        }

        self.draw_count += 1;

        Ok(())
    }
}
