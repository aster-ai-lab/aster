//! The migratable Studio cron subset: five fields, numeric values, lists,
//! ranges, steps, and English weekday names. There is deliberately no second
//! parser for seconds, macros, time-zone names, or month names.

use super::ScheduleError;
use chrono::{DateTime, Datelike, Duration, FixedOffset, Timelike};

const MINUTE: Duration = Duration::minutes(1);
const SEARCH_MINUTES: usize = 366 * 24 * 60;

#[derive(Clone, Copy)]
struct Span {
    start: u32,
    end: u32,
    step: u32,
}

struct Field {
    any: bool,
    spans: Vec<Span>,
}

impl Field {
    fn parse(raw: &str, min: u32, max: u32, weekday_names: bool) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        if raw == "*" {
            return Some(Self {
                any: true,
                spans: Vec::new(),
            });
        }
        let mut spans = Vec::new();
        for part in raw.split(',') {
            spans.push(parse_span(part, min, max, weekday_names)?);
        }
        Some(Self { any: false, spans })
    }

    fn matches(&self, value: u32) -> bool {
        self.any
            || self.spans.iter().any(|span| {
                value >= span.start && value <= span.end && (value - span.start) % span.step == 0
            })
    }
}

fn parse_span(part: &str, min: u32, max: u32, weekday_names: bool) -> Option<Span> {
    let (span, step) = match part.split_once('/') {
        Some((span, step)) => (span, step.parse::<u32>().ok()?),
        None => (part, 1),
    };
    if step == 0 {
        return None;
    }
    if span == "*" {
        return Some(Span {
            start: min,
            end: max,
            step,
        });
    }
    if let Some((start, end)) = span.split_once('-') {
        let start = parse_token(start, min, max, weekday_names)?;
        let end = parse_token(end, min, max, weekday_names)?;
        if start > end {
            return None;
        }
        return Some(Span { start, end, step });
    }
    let value = parse_token(span, min, max, weekday_names)?;
    Some(Span {
        start: value,
        end: value,
        step: 1,
    })
}

fn parse_token(raw: &str, min: u32, max: u32, weekday_names: bool) -> Option<u32> {
    let value = match (weekday_names, raw.to_ascii_uppercase().as_str()) {
        (true, "SUN") => 0,
        (true, "MON") => 1,
        (true, "TUE") => 2,
        (true, "WED") => 3,
        (true, "THU") => 4,
        (true, "FRI") => 5,
        (true, "SAT") => 6,
        _ => raw.parse().ok()?,
    };
    (value >= min && value <= max).then_some(value)
}

struct Cron {
    minute: Field,
    hour: Field,
    day: Field,
    month: Field,
    weekday: Field,
    day_any: bool,
    weekday_any: bool,
}

impl Cron {
    fn parse(expression: &str) -> Option<Self> {
        let parts: Vec<&str> = expression.split_whitespace().collect();
        if parts.len() != 5 {
            return None;
        }
        Some(Self {
            minute: Field::parse(parts[0], 0, 59, false)?,
            hour: Field::parse(parts[1], 0, 23, false)?,
            day: Field::parse(parts[2], 1, 31, false)?,
            month: Field::parse(parts[3], 1, 12, false)?,
            weekday: Field::parse(parts[4], 0, 7, true)?,
            day_any: parts[2] == "*",
            weekday_any: parts[4] == "*",
        })
    }

    fn matches(&self, when: DateTime<FixedOffset>) -> bool {
        if !self.minute.matches(when.minute())
            || !self.hour.matches(when.hour())
            || !self.month.matches(when.month())
        {
            return false;
        }
        let weekday = when.weekday().num_days_from_sunday();
        let weekday_ok = self.weekday.matches(weekday) || (weekday == 0 && self.weekday.matches(7));
        let day_ok = self.day.matches(when.day());
        if self.day_any && self.weekday_any {
            true
        } else if self.day_any {
            weekday_ok
        } else if self.weekday_any {
            day_ok
        } else {
            day_ok || weekday_ok
        }
    }
}

pub(crate) fn validate(expression: &str, utc_offset_minutes: i32) -> Result<(), ScheduleError> {
    if !(-1439..=1439).contains(&utc_offset_minutes) || Cron::parse(expression).is_none() {
        return Err(ScheduleError::InvalidCron(expression.to_owned()));
    }
    Ok(())
}

/// Return the first matching minute strictly after `after_unix_ms`.
pub(crate) fn next_after(
    expression: &str,
    utc_offset_minutes: i32,
    after_unix_ms: u64,
) -> Result<Option<u64>, ScheduleError> {
    validate(expression, utc_offset_minutes)?;
    let after_ms = i64::try_from(after_unix_ms)
        .map_err(|_| ScheduleError::InvalidField("next_due_unix_ms"))?;
    let after = DateTime::from_timestamp_millis(after_ms)
        .ok_or(ScheduleError::InvalidField("next_due_unix_ms"))?;
    let offset = FixedOffset::east_opt(utc_offset_minutes * 60)
        .ok_or_else(|| ScheduleError::InvalidCron(expression.to_owned()))?;
    let cron =
        Cron::parse(expression).ok_or_else(|| ScheduleError::InvalidCron(expression.to_owned()))?;
    let mut cursor = after
        .checked_add_signed(MINUTE)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .ok_or(ScheduleError::InvalidField("next_due_unix_ms"))?
        .with_timezone(&offset);
    for _ in 0..SEARCH_MINUTES {
        if cron.matches(cursor) {
            return u64::try_from(cursor.timestamp_millis())
                .map(Some)
                .map_err(|_| ScheduleError::InvalidField("next_due_unix_ms"));
        }
        cursor = cursor
            .checked_add_signed(MINUTE)
            .ok_or(ScheduleError::InvalidField("next_due_unix_ms"))?;
    }
    Ok(None)
}
