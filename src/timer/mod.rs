use std::os::fd::{AsRawFd, RawFd};
use std::path::Path;
use std::time::Duration;

use log::warn;
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};
use thiserror::Error;

use self::rtc::{RtcAlarm, RtcClock};

mod rtc;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Rtc(#[from] rtc::Error),
    #[error("Timer error")]
    Fd(#[from] nix::Error),
}
enum TimerImpl {
    Rtc(RtcClock),
    Fd(TimerFd),
}

pub struct Timer {
    timer: TimerImpl,
}

impl Timer {
    pub fn monotonic() -> Result<Self, Error> {
        Ok(Timer {
            timer: TimerImpl::Fd(TimerFd::new(
                ClockId::CLOCK_MONOTONIC,
                TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
            )?),
        })
    }

    pub fn realtime_alarm(path: impl AsRef<Path>) -> Result<Self, Error> {
        // Linux 3.11+ actually supports CLOCK_BOOTTIME_ALARM on TimerFd.
        // Unfortunately, the Kindle 4 ships with Linux 2.6.31, so rely on
        // manually programming an RTC device
        Ok(Timer {
            timer: TimerImpl::Rtc(RtcClock::new(path)?),
        })
    }

    pub fn set(&self, duration: Duration) -> Result<Alarm, Error> {
        match &self.timer {
            TimerImpl::Fd(timerfd) => {
                timerfd.set(
                    Expiration::Interval(TimeSpec::from_duration(duration)),
                    TimerSetTimeFlags::empty(),
                )?;
                Ok(Alarm {
                    alarm: AlarmImpl::Fd(timerfd),
                })
            }
            TimerImpl::Rtc(rtc_clock) => {
                let rtc_alarm = rtc_clock.set_alarm(duration)?;
                Ok(Alarm {
                    alarm: AlarmImpl::Rtc(rtc_alarm),
                })
            }
        }
    }
}

enum AlarmImpl<'a> {
    Rtc(RtcAlarm<'a>),
    Fd(&'a TimerFd),
}

pub struct Alarm<'a> {
    alarm: AlarmImpl<'a>,
}

impl Alarm<'_> {
    pub fn unset(&self) -> Result<(), Error> {
        match &self.alarm {
            AlarmImpl::Fd(timerfd) => Ok(timerfd.unset()?),
            AlarmImpl::Rtc(rtc_alarm) => Ok(rtc_alarm.unset()?),
        }
    }

    pub fn wait(&self) -> Result<(), Error> {
        match &self.alarm {
            AlarmImpl::Fd(timerfd) => Ok(timerfd.wait()?),
            AlarmImpl::Rtc(rtc_alarm) => Ok(rtc_alarm.wait()?),
        }
    }
}

impl AsRawFd for Alarm<'_> {
    fn as_raw_fd(&self) -> RawFd {
        match &self.alarm {
            AlarmImpl::Fd(timerfd) => timerfd.as_raw_fd(),
            AlarmImpl::Rtc(rtc_alarm) => rtc_alarm.as_raw_fd(),
        }
    }
}

impl Drop for Alarm<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.unset() {
            warn!("Failed to disable alarm: {}", err)
        }
    }
}
