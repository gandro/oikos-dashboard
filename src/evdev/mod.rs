use std::collections::HashSet;
use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{self, MaybeUninit};
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};

use glob::glob;
use log::debug;
use nix::errno::Errno;
use thiserror::Error;

mod sys;

pub use sys::KeyCode;

#[derive(Debug, Error)]
pub enum Error {
    #[error("OS error")]
    OsError(#[from] nix::Error),
    #[error("I/O error")]
    IoError(#[from] io::Error),
    #[error("Failed to open input device file: {0:?}")]
    OpenError(PathBuf, #[source] io::Error),
    #[error("Invalid input device path pattern")]
    PatternError(#[from] glob::PatternError),
    #[error("Failed to access input device")]
    GlobError(#[from] glob::GlobError),
    #[error("No matching input devices found")]
    NoInputDevicesFound,
}

struct BitSet(Box<[u8]>);

impl BitSet {
    fn with_size(bits: usize) -> Self {
        BitSet(vec![0u8; bits / 8 + 1].into_boxed_slice())
    }

    fn is_set(&self, key: usize) -> bool {
        self.0[key / 8] & (1 << (key % 8)) != 0
    }
}

impl Deref for BitSet {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BitSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub struct KeyDeviceBuilder {
    keys: Vec<KeyCode>,
}

impl KeyDeviceBuilder {
    pub fn with_keys(keys: impl IntoIterator<Item = KeyCode>) -> Self {
        KeyDeviceBuilder {
            keys: Vec::from_iter(keys),
        }
    }

    pub fn find(self, pattern: &str) -> Result<impl Iterator<Item = KeyDevice>, Error> {
        let mut devices = Vec::new();
        for path in glob(pattern)? {
            let path = path?;
            let dev = KeyDevice::open(&path)?;
            let Some(dev) = dev.with_filter(self.keys.iter().copied())? else {
                continue;
            };

            if log::log_enabled!(log::Level::Debug) {
                let name = dev.device_name()?;
                debug!("Opened evdev input device {:?}: {:?}", path, name);
            }

            devices.push(dev);
        }

        if devices.is_empty() {
            return Err(Error::NoInputDevicesFound);
        }

        Ok(devices.into_iter())
    }
}

pub struct KeyDevice {
    dev: File,
    filter: HashSet<KeyCode>,
}

impl KeyDevice {
    fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let dev = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|err| Error::OpenError(path.to_path_buf(), err))?;

        Ok(KeyDevice {
            dev: dev,
            filter: HashSet::new(),
        })
    }

    fn with_filter(mut self, keycodes: impl IntoIterator<Item = KeyCode>) -> Result<Option<Self>, Error> {
        let mut events = BitSet::with_size(sys::EV_CNT as usize);
        unsafe {
            sys::evdev_get_event_bits(self.dev.as_raw_fd(), &mut events)?;
        }

        if !events.is_set(sys::EV_KEY as usize) {
            return Ok(None);
        }

        let mut keys = BitSet::with_size(KeyCode::COUNT);
        unsafe {
            sys::evdev_get_event_key_bits(self.dev.as_raw_fd(), &mut keys)?;
        }

        // Set up filter for all supported events. Note that Linux 4.4+ would
        // allow us to push this filter into the kernel using EVIOCSMASK.
        // Unfortunately, the Kindle 4 ships with Linux 2.6.31, so we need to
        // filter in userspace instead.
        for key in keycodes {
            if keys.is_set(key.code() as usize) {
                self.filter.insert(key);
            }
        }

        if self.filter.is_empty() {
            return Ok(None);
        }

        Ok(Some(self))
    }

    fn device_name(&self) -> Result<String, Error> {
        let mut buf = [0u8; 128];
        unsafe {
            sys::evdev_get_name(self.dev.as_raw_fd(), &mut buf)?;
        };

        Ok(CStr::from_bytes_until_nul(&buf)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?
            .to_string_lossy()
            .to_string())
    }

    pub fn next_key_press(&self) -> Result<Option<KeyCode>, Error> {
        loop {
            // Loop until a keypress matches a filter or read returns EWOULDBLOCK
            let event = unsafe {
                let event_len = mem::size_of::<libc::input_event>();
                let mut event = MaybeUninit::<libc::input_event>::zeroed();
                let errno = libc::read(self.dev.as_raw_fd(), event.as_mut_ptr() as *mut _, event_len);
                match Errno::result(errno) {
                    Ok(n) if n == event_len as isize => event.assume_init(),
                    Ok(_) => Err(io::Error::from(io::ErrorKind::UnexpectedEof))?,
                    Err(Errno::EWOULDBLOCK) => return Ok(None),
                    Err(err) => Err(err)?,
                }
            };

            if event.type_ != sys::EV_KEY || event.value == 0 {
                continue; // not a keypress
            }

            let keycode = KeyCode::from(event.code);
            if self.filter.contains(&keycode) {
                return Ok(Some(keycode));
            }
        }
    }
}

impl AsRawFd for KeyDevice {
    fn as_raw_fd(&self) -> RawFd {
        self.dev.as_raw_fd()
    }
}
