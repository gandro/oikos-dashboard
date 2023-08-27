use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;

use log::debug;
use nix::poll::{poll, PollFd, PollFlags};
use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};
use thiserror::Error;

use crate::evdev::{self, KeyCode, KeyDevice};
use crate::timer::{self, Timer};

#[derive(Debug, Error)]
pub enum Error {
    #[error("OS error")]
    OsError(#[from] nix::Error),
    #[error("Failed to fetch key press event")]
    EvdevError(#[from] evdev::Error),
    #[error("Failed to set up timer")]
    TimerError(#[from] timer::Error),
    #[error("Failed to suspend via /sys/power/state")]
    SuspendError(#[from] io::Error),
}

#[derive(Clone, Debug)]
pub enum WakeupReason {
    IntervalTick,
    ExitKeyPressed(KeyCode),
}

pub struct Sleeper {
    timer: Timer,
    duration: Duration,
    wakeup_keys: HashMap<RawFd, KeyDevice>,
    suspend: bool,
    suspend_grace: Duration,
}

impl Sleeper {
    pub fn new(duration: Duration, timer: Timer) -> Self {
        Sleeper {
            timer: timer,
            duration: duration,
            wakeup_keys: HashMap::new(),
            suspend: false,
            suspend_grace: Default::default(),
        }
    }

    pub fn wakeup_keys(&mut self, key_devices: impl IntoIterator<Item = KeyDevice>) -> &mut Self {
        for device in key_devices {
            self.wakeup_keys.insert(device.as_raw_fd(), device);
        }
        self
    }

    pub fn suspend(&mut self, yes: bool) -> &mut Self {
        self.suspend = yes;
        self
    }

    pub fn suspend_grace(&mut self, period: Duration) -> &mut Self {
        self.suspend_grace = period;
        self
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    fn set_suspend_timer(&self, pollfd: &mut Vec<PollFd>) -> Result<(bool, Option<TimerFd>), Error> {
        if !self.suspend {
            return Ok((false, None));
        } else if self.suspend_grace.is_zero() {
            return Ok((true, None));
        }

        debug!("Waiting {:?} before suspending to memory", self.suspend_grace);
        let timer = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::TFD_NONBLOCK)?;
        let expiration = Expiration::OneShot(TimeSpec::from_duration(self.suspend_grace));
        timer.set(expiration, TimerSetTimeFlags::empty())?;

        pollfd.push(PollFd::new(timer.as_raw_fd(), PollFlags::POLLIN));

        Ok((false, Some(timer)))
    }

    fn suspend_to_memory(&self) -> Result<(), Error> {
        debug!("Suspending to memory");

        OpenOptions::new()
            .write(true)
            .open("/sys/power/state")?
            .write_all(b"mem")?;

        Ok(())
    }

    pub fn wait(&self) -> Result<WakeupReason, Error> {
        let wakeup_timer = self.timer.set(self.duration)?;
        let mut pollfd = vec![PollFd::new(wakeup_timer.as_raw_fd(), PollFlags::POLLIN)];

        for &fd in self.wakeup_keys.keys() {
            pollfd.push(PollFd::new(fd, PollFlags::POLLIN))
        }

        let (mut suspend_now, suspend_timer) = self.set_suspend_timer(&mut pollfd)?;

        loop {
            if suspend_now {
                self.suspend_to_memory()?;
                suspend_now = false;
            }

            poll(&mut pollfd, -1)?;

            for event in &pollfd {
                let fd = event.as_raw_fd();
                if !event.any().unwrap_or(false) {
                    continue;
                }

                if let Some(key) = self.wakeup_keys.get(&fd) {
                    if let Some(code) = key.next_key_press()? {
                        return Ok(WakeupReason::ExitKeyPressed(code));
                    }
                }

                if fd == wakeup_timer.as_raw_fd() {
                    wakeup_timer.wait()?;
                    return Ok(WakeupReason::IntervalTick);
                }

                if let Some(suspend_timer) = &suspend_timer {
                    if fd == suspend_timer.as_raw_fd() {
                        suspend_timer.wait()?;
                        suspend_now = true;
                    }
                }
            }
        }
    }
}
