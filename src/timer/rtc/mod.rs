use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader};
use std::mem::{self, MaybeUninit};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::prelude::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use log::debug;
use nix::errno::Errno;
use thiserror::Error;
use tz::{DateTime, TimeZoneRef, TzError};

mod sys;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Clock error")]
    ClockError(#[from] nix::Error),
    #[error("Failed to open RTC device")]
    RtcDeviceError(#[source] io::Error),
    #[error("Failed to access sysfs")]
    SysFsError(#[source] io::Error),
    #[error("Timestamp conversion error")]
    TzError(#[from] TzError),
    #[error("RTC device does not support wakeup alarms")]
    WakeupNotSupported,
    #[error("RTC uses local timezone, but no local timezone was found")]
    UnknownLocalTimezone,
    #[error("Unable to convert RTC time to local timezone time")]
    InvalidRTCTime,
}

#[derive(Copy, Clone, Debug)]
enum RealTimeClockMode {
    Utc,
    Local,
}

impl RealTimeClockMode {
    fn detect() -> Result<Self, io::Error> {
        let adjtime = BufReader::new(File::open("/etc/adjtime")?);
        let clock_mode = adjtime
            .lines()
            .nth(2)
            .ok_or(io::Error::from(io::ErrorKind::UnexpectedEof))??;
        match &*clock_mode {
            "UTC" => Ok(RealTimeClockMode::Utc),
            "LOCAL" => Ok(RealTimeClockMode::Local),
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}

pub struct RtcAlarm<'a> {
    dev: &'a File,
}

impl<'a> RtcAlarm<'a> {
    fn enable(rtc: &'a File, wakeup_time: sys::rtc_time) -> Result<Self, Error> {
        let fd = rtc.as_raw_fd();
        unsafe {
            sys::rtc_wkalrm_set(
                fd,
                &sys::rtc_wkalrm {
                    enabled: 1,
                    pending: 0,
                    time: wakeup_time,
                },
            )?;
        };
        Ok(RtcAlarm { dev: rtc })
    }

    pub fn unset(&self) -> Result<(), Error> {
        let fd = self.dev.as_raw_fd();
        let mut alarm = unsafe {
            let mut alarm = MaybeUninit::<sys::rtc_wkalrm>::zeroed();
            sys::rtc_wkalrm_rd(fd, alarm.as_mut_ptr())?;
            alarm.assume_init()
        };

        alarm.enabled = 0;
        unsafe {
            sys::rtc_wkalrm_set(fd, &alarm)?;
        };
        Ok(())
    }

    pub fn wait(&self) -> Result<(), Error> {
        let mut rtc_irq_data: libc::c_ulong = 0;
        let rtc_irq_data_len = mem::size_of_val(&rtc_irq_data);
        loop {
            let errno = unsafe {
                let rtc_irq_data_ptr = &mut rtc_irq_data as *mut libc::c_ulong;
                libc::read(self.dev.as_raw_fd(), rtc_irq_data_ptr as *mut _, rtc_irq_data_len)
            };
            match Errno::result(errno) {
                Ok(n) if n == rtc_irq_data_len as isize => {
                    if rtc_irq_data & (sys::RTC_AF as libc::c_ulong) != 0 {
                        return Ok(());
                    }
                }
                Ok(_) => return Err(Error::RtcDeviceError(io::Error::from(io::ErrorKind::UnexpectedEof))),
                Err(e) => return Err(Error::ClockError(e)),
            };
        }
    }
}

impl AsRawFd for RtcAlarm<'_> {
    fn as_raw_fd(&self) -> RawFd {
        self.dev.as_raw_fd()
    }
}

fn wakeup_supported(path: &Path) -> Result<bool, io::Error> {
    let rtc = path.file_name().ok_or(io::Error::from(io::ErrorKind::NotFound))?;
    let wakeup = PathBuf::from("/sys/class/rtc/").join(rtc).join("device/power/wakeup");
    Ok(fs::read_to_string(wakeup)?.trim() == "enabled")
}

#[derive(Debug)]
pub struct RtcClock {
    dev: File,
    tz: TimeZoneRef<'static>,
}

impl RtcClock {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Error> {
        let dev = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
            .map_err(Error::RtcDeviceError)?;

        if !wakeup_supported(path.as_ref()).map_err(Error::SysFsError)? {
            return Err(Error::WakeupNotSupported);
        }

        let rtc_mode = match RealTimeClockMode::detect() {
            Ok(rtc_mode) => rtc_mode,
            Err(err) => {
                debug!("Unable to auto-detect RTC clockmode: {}", err);
                RealTimeClockMode::Utc
            }
        };

        let tz = match rtc_mode {
            RealTimeClockMode::Local => tzdb::local_tz().ok_or(Error::UnknownLocalTimezone)?,
            RealTimeClockMode::Utc => tzdb::time_zone::UTC,
        };

        debug!("Using RTC {:?} with {:?} clockmode", path.as_ref(), rtc_mode);
        Ok(RtcClock { dev, tz })
    }

    fn rtc_time(&self) -> Result<DateTime, Error> {
        let fd = self.dev.as_raw_fd();
        let rtc_time = unsafe {
            let mut time = MaybeUninit::<sys::rtc_time>::zeroed();
            sys::rtc_rd_time(fd, time.as_mut_ptr())?;
            time.assume_init()
        };

        DateTime::find(
            rtc_time.tm_year as i32 + 1900,
            rtc_time.tm_mon as u8 + 1,
            rtc_time.tm_mday as u8,
            rtc_time.tm_hour as u8,
            rtc_time.tm_min as u8,
            rtc_time.tm_sec as u8,
            0,
            self.tz,
        )?
        .earliest()
        .ok_or(Error::InvalidRTCTime)
    }

    fn rtc_time_add_duration(&self, duration: Duration) -> Result<sys::rtc_time, Error> {
        let duration: i64 = duration.as_secs().try_into().map_err(TzError::from)?;
        let rtc_unixtime = self.rtc_time()?.unix_time() + duration;
        let rtc_datetime = DateTime::from_timespec(rtc_unixtime, 0, self.tz).map_err(TzError::from)?;

        Ok(sys::rtc_time {
            tm_sec: rtc_datetime.second() as std::ffi::c_int,
            tm_min: rtc_datetime.minute() as std::ffi::c_int,
            tm_hour: rtc_datetime.hour() as std::ffi::c_int,
            tm_mday: rtc_datetime.month_day() as std::ffi::c_int,
            tm_mon: rtc_datetime.month() as std::ffi::c_int - 1,
            tm_year: rtc_datetime.year() as std::ffi::c_int - 1900,
            tm_wday: -1,  // unused
            tm_yday: -1,  // unused
            tm_isdst: -1, // unused
        })
    }

    pub fn set_alarm<'a>(&'a self, duration: Duration) -> Result<RtcAlarm<'a>, Error> {
        let wakeup_time = self.rtc_time_add_duration(duration)?;
        RtcAlarm::enable(&self.dev, wakeup_time)
    }
}
