//! Display formatting for spreadsheet cell text views and annotated reads.

/// Apply a limited, deterministic subset of Excel number formats for agent text.
/// Unknown formats leave the raw stored value unchanged.
pub(in crate::semantic) fn format_number_display(raw: &str, number_format: &str) -> Option<String> {
    let format = number_format.trim();
    if format.is_empty() || format.eq_ignore_ascii_case("General") || format == "@" {
        return None;
    }
    let value = raw.parse::<f64>().ok()?;
    if !value.is_finite() {
        return None;
    }
    if let Some(decimals) = percent_decimals(format) {
        return Some(format_percent(value, decimals));
    }
    if is_date_format(format) {
        return excel_serial_to_ymd(value)
            .map(|(year, month, day)| format!("{year:04}-{month:02}-{day:02}"));
    }
    if let Some(decimals) = fixed_decimals(format) {
        return Some(format_fixed(value, decimals));
    }
    None
}

fn percent_decimals(format: &str) -> Option<usize> {
    let trimmed = format.trim();
    if !trimmed.ends_with('%') {
        return None;
    }
    let body = trimmed.trim_end_matches('%').trim();
    if body == "0" {
        return Some(0);
    }
    if let Some(fraction) = body.strip_prefix("0.") {
        if !fraction.is_empty() && fraction.bytes().all(|byte| byte == b'0') {
            return Some(fraction.len());
        }
    }
    None
}

fn fixed_decimals(format: &str) -> Option<usize> {
    if format == "0" {
        return Some(0);
    }
    let Some(fraction) = format.strip_prefix("0.") else {
        return None;
    };
    if !fraction.is_empty() && fraction.bytes().all(|byte| byte == b'0') {
        Some(fraction.len())
    } else {
        None
    }
}

fn is_date_format(format: &str) -> bool {
    let lower = format.to_ascii_lowercase();
    lower.contains("yy") && (lower.contains('m') || lower.contains('d'))
}

fn format_percent(value: f64, decimals: usize) -> String {
    let scaled = value * 100.0;
    let mut text = format_fixed(scaled, decimals);
    text.push('%');
    text
}

fn format_fixed(value: f64, decimals: usize) -> String {
    let factor = 10_f64.powi(decimals as i32);
    let rounded = (value * factor).round() / factor;
    if decimals == 0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.decimals$}")
    }
}

fn excel_serial_to_ymd(serial: f64) -> Option<(i32, u32, u32)> {
    if !(0.0..2_958_466.0).contains(&serial) {
        return None;
    }
    let days = serial.floor() as i64;
    // Excel's 1900 leap-year bug is absorbed by the 1899-12-30 baseline for
    // civil dates on or after 1900-03-01, matching the import serializer.
    civil_from_days(days_from_civil(1899, 12, 30) + days)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i32::try_from(month).unwrap_or_default();
    let day = i32::try_from(day).unwrap_or_default();
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era)
}

fn civil_from_days(days: i64) -> Option<(i32, u32, u32)> {
    let days = i32::try_from(days).ok()?;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i32::from(month <= 2);
    Some((year, month as u32, day as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_percent_and_excel_date_serials() {
        assert_eq!(
            format_number_display("0.125", "0.0%").as_deref(),
            Some("12.5%")
        );
        assert_eq!(
            format_number_display("46327", "yyyy-mm-dd").as_deref(),
            Some("2026-11-01")
        );
        assert_eq!(format_number_display("0.1", "0%").as_deref(), Some("10%"));
        assert_eq!(
            format_number_display("465", "0.00").as_deref(),
            Some("465.00")
        );
    }
}
