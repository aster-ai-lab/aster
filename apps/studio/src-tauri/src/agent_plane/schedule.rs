//! Next-run math for the five-field cron expressions the task form emits,
//! plus interval and one-shot triggers. Times are computed in the caller's
//! offset so a saved "09:00" means 09:00 local, and tests can pin UTC.

use chrono::{DateTime, Datelike, Duration, FixedOffset, Timelike};

const MINUTE: Duration = Duration::minutes(1);
/// A slot older than this is marked missed instead of catching up.
pub const CATCH_UP: Duration = Duration::hours(24);

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
    fn parse(raw: &str, min: u32, max: u32) -> Option<Self> {
        let raw = raw.trim();
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
            spans.push(parse_span(part.trim(), min, max)?);
        }
        Some(Self { any: false, spans })
    }

    fn matches(&self, value: u32) -> bool {
        self.any
            || self.spans.iter().any(|span| {
                value >= span.start
                    && value <= span.end
                    && (value - span.start).is_multiple_of(span.step)
            })
    }
}

fn parse_span(part: &str, min: u32, max: u32) -> Option<Span> {
    let (span, step) = match part.split_once('/') {
        Some((span, step)) => (span, step.parse::<u32>().ok()?.max(1)),
        None => (part, 1),
    };
    if span == "*" {
        return Some(Span {
            start: min,
            end: max,
            step,
        });
    }
    if let Some((start, end)) = span.split_once('-') {
        let start = parse_token(start, min, max)?;
        let end = parse_token(end, min, max)?;
        if start > end {
            return None;
        }
        return Some(Span { start, end, step });
    }
    let value = parse_token(span, min, max)?;
    Some(Span {
        start: value,
        end: value,
        step: 1,
    })
}

fn parse_token(raw: &str, min: u32, max: u32) -> Option<u32> {
    let value = match raw.to_ascii_uppercase().as_str() {
        "SUN" => 0,
        "MON" => 1,
        "TUE" => 2,
        "WED" => 3,
        "THU" => 4,
        "FRI" => 5,
        "SAT" => 6,
        other => other.parse().ok()?,
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
    fn parse(expr: &str) -> Option<Self> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return None;
        }
        Some(Self {
            minute: Field::parse(parts[0], 0, 59)?,
            hour: Field::parse(parts[1], 0, 23)?,
            day: Field::parse(parts[2], 1, 31)?,
            month: Field::parse(parts[3], 1, 12)?,
            weekday: Field::parse(parts[4], 0, 7)?,
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

/// The first minute strictly after `after` that matches `expr`.
pub fn next_cron(expr: &str, after: DateTime<FixedOffset>) -> Option<DateTime<FixedOffset>> {
    let cron = Cron::parse(expr)?;
    let mut cursor = after + MINUTE;
    cursor = cursor.with_second(0)?.with_nanosecond(0)?;
    for _ in 0..(366 * 24 * 60) {
        if cron.matches(cursor) {
            return Some(cursor);
        }
        cursor += MINUTE;
    }
    None
}

pub fn parse_instant(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

pub fn format_instant(value: DateTime<FixedOffset>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// `None` means the trigger does not schedule another fire (a finished one-shot).
pub fn next_trigger(
    trigger: &serde_json::Value,
    after: DateTime<FixedOffset>,
) -> Option<Option<String>> {
    let kind = trigger.get("kind")?.as_str()?;
    match kind {
        "cron" => {
            let expr = trigger.get("expr")?.as_str()?;
            Some(Some(format_instant(next_cron(expr, after)?)))
        }
        "interval" => {
            let ms = trigger.get("ms")?.as_i64()?;
            if ms < 1_000 {
                return None;
            }
            Some(Some(format_instant(after + Duration::milliseconds(ms))))
        }
        "once" => {
            let at = trigger.get("at")?.as_i64()?;
            DateTime::from_timestamp_millis(at)
                .map(|instant| Some(format_instant(instant.fixed_offset())))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<FixedOffset> {
        chrono::Utc
            .with_ymd_and_hms(2026, 10, day, hour, minute, 0)
            .unwrap()
            .fixed_offset()
    }

    #[test]
    fn hourly_and_weekday_crons_land_on_the_next_minute() {
        // 2026-10-05 is a Monday. 01:30 → 02:00.
        let next = next_cron("0 * * * *", at(5, 1, 30)).unwrap();
        assert_eq!((next.day(), next.hour(), next.minute()), (5, 2, 0));

        // Monday 11:00 is past 10:30, so the next weekday slot is Tuesday.
        let tuesday = next_cron("30 10 * * 1-5", at(5, 11, 0)).unwrap();
        assert_eq!(tuesday.weekday().num_days_from_sunday(), 2);
        assert_eq!((tuesday.hour(), tuesday.minute()), (10, 30));
    }

    #[test]
    fn named_weekdays_and_hour_lists_match() {
        let next = next_cron("*/15 9-17 * * MON-FRI", at(4, 16, 0)).unwrap();
        assert_eq!(next.weekday().num_days_from_sunday(), 1);
        assert_eq!((next.hour(), next.minute()), (9, 0));

        let later = next_cron("30 9,18 * * *", at(5, 5, 0)).unwrap();
        assert_eq!((later.day(), later.hour(), later.minute()), (5, 9, 30));
    }

    #[test]
    fn rejects_a_blank_or_short_expression() {
        assert!(next_cron("nope", at(5, 1, 30)).is_none());
        assert!(next_cron("0 * * *", at(5, 1, 30)).is_none());
    }
}
