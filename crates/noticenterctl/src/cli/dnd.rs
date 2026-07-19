//! Timed Do Not Disturb command value parsing and deadline resolution

use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::{Days, Local, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};

// Relative durations stay bounded so persisted deadlines remain operationally useful
const MAX_DND_DURATION_SECONDS: u64 = 365 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DndDuration {
    seconds: u64,
}

impl DndDuration {
    pub fn deadline(self) -> Result<i64> {
        // The daemon receives one absolute timestamp so restarts do not reset the duration
        Utc::now()
            .timestamp()
            .checked_add_unsigned(self.seconds)
            .ok_or_else(|| anyhow!("DND duration exceeds the supported timestamp range"))
    }
}

impl FromStr for DndDuration {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        // The final ASCII byte selects the only supported duration unit
        let Some(unit) = value.as_bytes().last().copied() else {
            return Err("duration cannot be empty".to_string());
        };
        let multiplier = match unit {
            b's' => 1,
            b'm' => 60,
            b'h' => 60 * 60,
            b'd' => 24 * 60 * 60,
            _ => return Err("duration must end in s, m, h, or d".to_string()),
        };
        // Supported suffixes are one-byte ASCII, so this boundary is always valid
        let digits = &value[..value.len() - 1];
        let amount = digits
            .parse::<u64>()
            .map_err(|_| "duration must start with a positive integer".to_string())?;
        // Checked multiplication rejects large values before the policy bound is applied
        let seconds = amount
            .checked_mul(multiplier)
            .ok_or_else(|| "duration is too large".to_string())?;
        if seconds == 0 || seconds > MAX_DND_DURATION_SECONDS {
            return Err("duration must be between 1 second and 365 days".to_string());
        }
        Ok(Self { seconds })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DndClockTime {
    time: NaiveTime,
}

impl DndClockTime {
    pub fn deadline(self) -> Result<i64> {
        // Resolve against the machine timezone because HH:MM is a local wall-clock value
        let now = Local::now();
        let now_timestamp = now.timestamp();
        if let Some(deadline) = local_deadline_after(now.date_naive(), self.time, now_timestamp) {
            // A repeated local hour may still have a second future occurrence today
            return Ok(deadline);
        }

        // A missing or elapsed occurrence today advances by one calendar day
        let tomorrow = tomorrow_date(now.date_naive())?;
        local_deadline_after(tomorrow, self.time, now_timestamp)
            .ok_or_else(|| anyhow!("requested local time does not exist on the next calendar date"))
    }
}

impl FromStr for DndClockTime {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        let bytes = value.as_bytes();
        // Exact width avoids accepting locale-specific or partly parsed clock forms
        if bytes.len() != 5
            || bytes[2] != b':'
            || !bytes[..2].iter().all(u8::is_ascii_digit)
            || !bytes[3..].iter().all(u8::is_ascii_digit)
        {
            return Err("time must use 24-hour HH:MM format".to_string());
        }
        let time = NaiveTime::parse_from_str(value, "%H:%M")
            .map_err(|_| "time must use 24-hour HH:MM format".to_string())?;
        Ok(Self { time })
    }
}

fn tomorrow_date(today: NaiveDate) -> Result<NaiveDate> {
    // Calendar addition remains correct across daylight-saving offset changes
    today
        .checked_add_days(Days::new(1))
        .ok_or_else(|| anyhow!("next DND date exceeds the supported calendar range"))
}

fn local_deadline_after(date: NaiveDate, time: NaiveTime, after: i64) -> Option<i64> {
    let local = Local.from_local_datetime(&date.and_time(time));
    match local {
        LocalResult::Single(value) => future_timestamp(after, Some(value.timestamp()), None),
        // Repeated hours expose both absolute instants for future filtering
        LocalResult::Ambiguous(first, second) => {
            future_timestamp(after, Some(first.timestamp()), Some(second.timestamp()))
        }
        // A skipped wall-clock time has no deadline on this date
        LocalResult::None => None,
    }
}

pub(super) fn future_timestamp(after: i64, first: Option<i64>, second: Option<i64>) -> Option<i64> {
    // Select by absolute time so repeated wall-clock hours remain correct
    [first, second]
        .into_iter()
        .flatten()
        .filter(|candidate| *candidate > after)
        .min()
}
