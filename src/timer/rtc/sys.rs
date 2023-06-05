use nix::{ioctl_read, ioctl_write_ptr};

const RTC_IOC_MAGIC: u8 = b'p';

const RTC_RD_TIME: u8 = 0x09;

const RTC_WKALM_RD: u8 = 0x10;
const RTC_WKALM_SET: u8 = 0x0f;

pub const RTC_AF: u8 = 0x20;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct rtc_time {
    // Seconds (0-60)
    pub tm_sec: std::ffi::c_int,
    // Minutes (0-59)
    pub tm_min: std::ffi::c_int,
    // Hours (0-23)
    pub tm_hour: std::ffi::c_int,
    // Day of the month (1-31)
    pub tm_mday: std::ffi::c_int,
    // Month (0-11)
    pub tm_mon: std::ffi::c_int,
    // Year - 1900
    pub tm_year: std::ffi::c_int,
    // Day of the week (0-6, Sunday = 0)
    pub tm_wday: std::ffi::c_int,
    // Day in the year (0-365, 1 Jan = 0)
    pub tm_yday: std::ffi::c_int,
    // Daylight saving time
    pub tm_isdst: std::ffi::c_int,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct rtc_wkalrm {
    pub enabled: std::ffi::c_uchar,
    pub pending: std::ffi::c_uchar,
    pub time: rtc_time,
}

ioctl_read!(rtc_rd_time, RTC_IOC_MAGIC, RTC_RD_TIME, rtc_time);
ioctl_write_ptr!(rtc_wkalrm_set, RTC_IOC_MAGIC, RTC_WKALM_SET, rtc_wkalrm);
ioctl_read!(rtc_wkalrm_rd, RTC_IOC_MAGIC, RTC_WKALM_RD, rtc_wkalrm);
