use std::env;

use rhai::plugin::*;
use tz;
use tzdb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeDelta(pub i128);

impl TimeDelta {
    pub fn as_nanoseconds(&self) -> i128 {
        self.0
    }
}

fn local_tz() -> tz::TimeZoneRef<'static> {
    tzdb::local_tz()
        .or_else(|| tzdb::tz_by_name(env::var("TZ").unwrap_or_default()))
        .unwrap_or(tzdb::time_zone::UTC)
}

#[export_module]
pub mod datetime {
    pub type DateTime = tz::DateTime;

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn now() -> Result<DateTime, Box<EvalAltResult>> {
        tzdb::now::in_tz(local_tz()).map_err(|e| e.to_string().into())
    }

    #[rhai_fn(return_raw)]
    pub fn from_date_string(s: &str) -> Result<DateTime, Box<EvalAltResult>> {
        let mut components = s.splitn(3, "-").flat_map(|s| s.parse::<i64>().ok());
        let year = components.next().ok_or("missing year")?;
        let month = components.next().ok_or("missing month")?;
        let day = components.next().ok_or("missing day")?;
        from_date(year, month, day)
    }

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn from_date(year: i64, month: i64, day: i64) -> Result<DateTime, Box<EvalAltResult>> {
        from_date_and_time(year, month, day, 0, 0, 0)
    }

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn from_date_and_time(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
    ) -> Result<DateTime, Box<EvalAltResult>> {
        from_date_and_time_with_ns_and_tz(year, month, day, hour, minute, second, 0, local_tz())
    }

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn from_date_and_time_with_tz_str(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        tz: &str,
    ) -> Result<DateTime, Box<EvalAltResult>> {
        let time_zone = tzdb::tz_by_name(tz).ok_or(EvalAltResult::from(format!("timezone {:?} not found", tz)))?;
        from_date_and_time_with_ns_and_tz(year, month, day, hour, minute, second, 0, time_zone)
    }

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn from_date_and_time_with_ns(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        nanoseconds: i64,
    ) -> Result<DateTime, Box<EvalAltResult>> {
        from_date_and_time_with_ns_and_tz(year, month, day, hour, minute, second, nanoseconds, local_tz())
    }

    #[rhai_fn(return_raw, name = "datetime")]
    pub fn from_date_and_time_with_ns_and_tz_str(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        nanoseconds: i64,
        tz: &str,
    ) -> Result<DateTime, Box<EvalAltResult>> {
        let time_zone = tzdb::tz_by_name(tz).ok_or(EvalAltResult::from(format!("timezone {:?} not found", tz)))?;
        from_date_and_time_with_ns_and_tz(year, month, day, hour, minute, second, nanoseconds, time_zone)
    }

    fn from_date_and_time_with_ns_and_tz(
        year: i64,
        month: i64,
        day: i64,
        hour: i64,
        minute: i64,
        second: i64,
        nanoseconds: i64,
        tz: tz::TimeZoneRef,
    ) -> Result<DateTime, Box<EvalAltResult>> {
        let year = year.try_into().map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let month = month.try_into().map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let day = day.try_into().map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let hour = hour.try_into().map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let minute = minute
            .try_into()
            .map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let second = second
            .try_into()
            .map_err(|e: std::num::TryFromIntError| e.to_string())?;
        let nanoseconds = nanoseconds
            .try_into()
            .map_err(|e: std::num::TryFromIntError| e.to_string())?;

        DateTime::find(year, month, day, hour, minute, second, nanoseconds, tz)
            .map_err(|e| e.to_string())?
            .unique()
            .ok_or("datetime is not unique".into())
    }

    #[rhai_fn(get = "year")]
    pub fn year(dt: &mut DateTime) -> i64 {
        dt.year() as i64
    }

    #[rhai_fn(get = "month")]
    pub fn month(dt: &mut DateTime) -> i64 {
        dt.month() as i64
    }
    #[rhai_fn(get = "month_day")]
    pub fn month_day(dt: &mut DateTime) -> i64 {
        dt.month_day() as i64
    }

    #[rhai_fn(get = "week_day")]
    pub fn week_day(dt: &mut DateTime) -> i64 {
        dt.week_day() as i64
    }

    #[rhai_fn(get = "hour")]
    pub fn hour(dt: &mut DateTime) -> i64 {
        dt.hour() as i64
    }

    #[rhai_fn(get = "minute")]
    pub fn minute(dt: &mut DateTime) -> i64 {
        dt.minute() as i64
    }

    #[rhai_fn(get = "second")]
    pub fn second(dt: &mut DateTime) -> i64 {
        dt.second() as i64
    }

    #[rhai_fn(get = "nanoseconds")]
    pub fn nanoseconds(dt: &mut DateTime) -> i64 {
        dt.nanoseconds() as i64
    }

    #[rhai_fn(name = "+", return_raw, global)]
    pub fn add_timedelta(dt: &mut DateTime, td: TimeDelta) -> Result<DateTime, Box<EvalAltResult>> {
        let unix = dt
            .total_nanoseconds()
            .checked_add(td.as_nanoseconds())
            .ok_or("timedelta overflow")?;
        DateTime::from_total_nanoseconds_and_local(unix, dt.local_time_type().clone()).map_err(|e| e.to_string().into())
    }

    #[rhai_fn(name = "-", return_raw, global)]
    pub fn sub_timedelta(dt: &mut DateTime, td: TimeDelta) -> Result<DateTime, Box<EvalAltResult>> {
        let unix = dt
            .total_nanoseconds()
            .checked_sub(td.as_nanoseconds())
            .ok_or("timedelta underflow")?;
        DateTime::from_total_nanoseconds_and_local(unix, dt.local_time_type().clone()).map_err(|e| e.to_string().into())
    }

    #[rhai_fn(name = "-", return_raw, global)]
    pub fn sub_datetime(dt: DateTime, other: DateTime) -> Result<TimeDelta, Box<EvalAltResult>> {
        dt.total_nanoseconds()
            .checked_sub(other.total_nanoseconds())
            .ok_or("timedelta overflow".into())
            .map(TimeDelta)
    }

    #[rhai_fn(pure, global)]
    pub fn to_string(dt: &mut DateTime) -> String {
        dt.to_string()
    }

    #[rhai_fn(pure, global)]
    pub fn to_date_string(dt: &mut DateTime) -> String {
        format!("{}-{:02}-{:02}", dt.year(), dt.month(), dt.month_day())
    }

    #[rhai_fn(pure, global)]
    pub fn to_debug(dt: &mut DateTime) -> String {
        format!(
            "datetime({}, {}, {}, {}, {}, {}, {}, {:?})",
            dt.year(),
            dt.month(),
            dt.month_day(),
            dt.hour(),
            dt.minute(),
            dt.second(),
            dt.nanoseconds(),
            dt.local_time_type().time_zone_designation()
        )
    }
}

#[export_module]
pub mod timedelta {
    pub type TimeDelta = super::TimeDelta;

    pub const NANOSECOND: TimeDelta = nanoseconds(1);
    pub const MICROSECOND: TimeDelta = microseconds(1);
    pub const MILLISECOND: TimeDelta = milliseconds(1);
    pub const SECOND: TimeDelta = seconds(1);
    pub const MINUTE: TimeDelta = minutes(1);
    pub const HOUR: TimeDelta = hours(1);
    pub const DAY: TimeDelta = days(1);

    pub const fn nanoseconds(i: i64) -> TimeDelta {
        TimeDelta(i as i128)
    }

    pub const fn microseconds(i: i64) -> TimeDelta {
        nanoseconds(1000 * i)
    }

    pub const fn milliseconds(i: i64) -> TimeDelta {
        microseconds(1000 * i)
    }

    pub const fn seconds(i: i64) -> TimeDelta {
        milliseconds(1000 * i)
    }

    pub const fn minutes(i: i64) -> TimeDelta {
        seconds(60 * i)
    }

    pub const fn hours(i: i64) -> TimeDelta {
        minutes(60 * i)
    }

    pub const fn days(i: i64) -> TimeDelta {
        hours(24 * i)
    }

    #[rhai_fn(name = "+", return_raw, global)]
    pub fn add(td: TimeDelta, other: TimeDelta) -> Result<TimeDelta, Box<EvalAltResult>> {
        td.as_nanoseconds()
            .checked_add(other.0)
            .ok_or("timedelta addition overflow".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "-", return_raw, global)]
    pub fn sub(td: TimeDelta, other: TimeDelta) -> Result<TimeDelta, Box<EvalAltResult>> {
        td.as_nanoseconds()
            .checked_sub(other.0)
            .ok_or("timedelta subtraction underflow".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "*", return_raw, global)]
    pub fn mul_int_rhs(td: TimeDelta, int: rhai::INT) -> Result<TimeDelta, Box<EvalAltResult>> {
        td.as_nanoseconds()
            .checked_mul(int as i128)
            .ok_or("timedelta multiplication overflow".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "*", return_raw, global)]
    pub fn mul_int_lhs(int: rhai::INT, td: TimeDelta) -> Result<TimeDelta, Box<EvalAltResult>> {
        (int as i128)
            .checked_mul(td.as_nanoseconds())
            .ok_or("timedelta multiplication overflow".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "/", return_raw, global)]
    pub fn div_int_rhs(td: TimeDelta, int: rhai::INT) -> Result<TimeDelta, Box<EvalAltResult>> {
        td.as_nanoseconds()
            .checked_div(int as i128)
            .ok_or("timedelta division error".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "/", return_raw, global)]
    pub fn div_int_lhs(int: rhai::INT, td: TimeDelta) -> Result<TimeDelta, Box<EvalAltResult>> {
        (int as i128)
            .checked_div(td.as_nanoseconds())
            .ok_or("timedelta division error".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "/", return_raw, global)]
    pub fn div(td: TimeDelta, other: TimeDelta) -> Result<TimeDelta, Box<EvalAltResult>> {
        td.as_nanoseconds()
            .checked_div(other.as_nanoseconds())
            .ok_or("timedelta division error".into())
            .map(TimeDelta)
    }

    #[rhai_fn(name = "==", global)]
    pub fn eq(td: TimeDelta, other: TimeDelta) -> bool {
        td == other
    }

    #[rhai_fn(name = "!=", global)]
    pub fn neq(td: TimeDelta, other: TimeDelta) -> bool {
        td != other
    }

    #[rhai_fn(name = "<", global)]
    pub fn lt(td: TimeDelta, other: TimeDelta) -> bool {
        td < other
    }

    #[rhai_fn(name = "<=", global)]
    pub fn le(td: TimeDelta, other: TimeDelta) -> bool {
        td <= other
    }

    #[rhai_fn(name = ">", global)]
    pub fn gt(td: TimeDelta, other: TimeDelta) -> bool {
        td > other
    }

    #[rhai_fn(name = ">=", global)]
    pub fn ge(td: TimeDelta, other: TimeDelta) -> bool {
        td >= other
    }

    #[rhai_fn(pure, global)]
    pub fn to_string(td: &mut TimeDelta) -> String {
        use std::fmt::Write;
        let mut fmt = String::new();

        let mut nanos = td.as_nanoseconds();
        if nanos.is_negative() {
            write!(fmt, "-").unwrap();
            nanos = nanos.abs();
        }

        let days = nanos / DAY.as_nanoseconds();
        if days != 0 {
            write!(fmt, "{}d", days).unwrap();
        }

        let hours = nanos % DAY.as_nanoseconds() / HOUR.as_nanoseconds();
        if hours != 0 {
            write!(fmt, "{:02}h", hours).unwrap();
        }

        let minutes = nanos % MINUTE.as_nanoseconds() / HOUR.as_nanoseconds();
        if minutes != 0 {
            write!(fmt, "{:02}m", minutes).unwrap();
        }

        let seconds = nanos % SECOND.as_nanoseconds() / MINUTE.as_nanoseconds();
        let subseconds = nanos % SECOND.as_nanoseconds();
        if seconds != 0 || subseconds != 0 {
            write!(fmt, "{:02}", seconds).unwrap();
            if subseconds != 0 {
                write!(fmt, ".{:09}", subseconds).unwrap();
            }
            write!(fmt, "s").unwrap();
        }

        fmt
    }

    #[rhai_fn(pure, global)]
    pub fn to_debug(td: &mut TimeDelta) -> String {
        format!("timedelta({})", td.0)
    }
}
