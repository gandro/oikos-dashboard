use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::thread;
use std::time::Duration;

use log::debug;
use nix::poll::{poll, PollFd, PollFlags};
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

    fn suspend_to_memory(&self) -> Result<(), Error> {
        debug!("Waiting {:?} before suspending to memory", self.suspend_grace);

        thread::sleep(self.suspend_grace);
        OpenOptions::new()
            .write(true)
            .open("/sys/power/state")?
            .write_all(b"mem")?;

        Ok(())
    }

    pub fn wait(&self) -> Result<WakeupReason, Error> {
        let alarm = self.timer.set(self.duration)?;
        let alarm_fd = alarm.as_raw_fd();
        let mut pollfd = vec![PollFd::new(alarm_fd, PollFlags::POLLIN)];
        for &fd in self.wakeup_keys.keys() {
            pollfd.push(PollFd::new(fd, PollFlags::POLLIN))
        }

        if self.suspend {
            self.suspend_to_memory()?;
        }

        loop {
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

                if fd == alarm_fd {
                    alarm.wait()?;
                    return Ok(WakeupReason::IntervalTick);
                }
            }
        }
    }
}
