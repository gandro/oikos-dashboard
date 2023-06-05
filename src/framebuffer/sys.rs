use nix::{ioctl_read_bad, ioctl_write_int_bad};

// Based on https://www.kernel.org/doc/Documentation/fb/api.txt
const FBIOGET_VSCREENINFO: u32 = 0x4600;
const FBIOGET_FSCREENINFO: u32 = 0x4602;

// Based on include/linux/einkfb.h from the Lab126 Linux 2.6.31 sources
const FBIO_EINK_UPDATE_DISPLAY: u16 = 0x46db;

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone)]
pub enum fx_type {
    fx_update_partial = 0,
    fx_update_full = 1,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fb_fix_screeninfo {
    // Identification string eg "TT Builtin"
    pub id: [std::ffi::c_char; 16],
    // Start of frame buffer mem (physical address)
    pub smem_start: std::ffi::c_ulong,
    // Length of frame buffer mem
    pub smem_len: u32,
    // Macropixel type
    pub type_: u32,
    // Interleave for interleaved Planes
    pub type_aux: u32,
    // Macropixels visual type
    pub visual: u32,
    // Zero if no hardware panning
    pub xpanstep: u16,
    // Zero if no hardware panning
    pub ypanstep: u16,
    // Zero if no hardware ywrap
    pub ywrapstep: u16,
    // Length of a line in bytes
    pub line_length: u32,
    // Start of Memory Mapped I/O (physical address)
    pub mmio_start: std::ffi::c_ulong,
    // Length of Memory Mapped I/O
    pub mmio_len: u32,
    // Indicate to driver which	specific chip/card we have
    pub accel: u32,
    // Device and driver capabilities
    pub capabilities: u16,
    // Reserved for future compatibility
    pub reserved: [u16; 2],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fb_bitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct fb_var_screeninfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub red: fb_bitfield,
    pub green: fb_bitfield,
    pub blue: fb_bitfield,
    pub transp: fb_bitfield,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
    pub accel_flags: u32,
    pub pixclock: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub upper_margin: u32,
    pub lower_margin: u32,
    pub hsync_len: u32,
    pub vsync_len: u32,
    pub sync: u32,
    pub vmode: u32,
    pub rotate: u32,
    pub colorspace: u32,
    pub reserved: [u32; 4],
}

ioctl_read_bad!(fb_get_fix_screeninfo, FBIOGET_FSCREENINFO, fb_fix_screeninfo);
ioctl_read_bad!(fb_get_var_screeninfo, FBIOGET_VSCREENINFO, fb_var_screeninfo);

ioctl_write_int_bad!(fbio_eink_update_display, FBIO_EINK_UPDATE_DISPLAY);
