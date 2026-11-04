//! Pure, bounded-output measurements used by the delivery checker.

#[derive(Clone, Copy)]
pub(super) enum EndpointCounting {
    Elapsed,
    Inclusive,
}

impl EndpointCounting {
    pub(super) fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "elapsed" => Ok(Self::Elapsed),
            "inclusive" => Ok(Self::Inclusive),
            _ => Err("endpoint_counting must be elapsed or inclusive"),
        }
    }
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Elapsed => "elapsed",
            Self::Inclusive => "inclusive",
        }
    }
}

pub(super) fn date_span(
    start: &str,
    end: &str,
    counting: EndpointCounting,
) -> Result<i64, &'static str> {
    use chrono::{Datelike, NaiveDate};
    let parse = |text: &str| {
        if text.len() != 10 {
            return Err("dates must use exact YYYY-MM-DD calendar dates");
        }
        let date =
            NaiveDate::parse_from_str(text, "%Y-%m-%d").map_err(|_| "invalid calendar date")?;
        if date.year() < 1 || date.format("%Y-%m-%d").to_string() != text {
            return Err("dates must use exact YYYY-MM-DD calendar dates");
        }
        Ok(date)
    };
    let start = parse(start)?;
    let end = parse(end)?;
    if end < start {
        return Err("end date precedes start date");
    }
    let elapsed = (end - start).num_days();
    Ok(elapsed
        + match counting {
            EndpointCounting::Elapsed => 0,
            EndpointCounting::Inclusive => 1,
        })
}

pub(super) fn text_counts(text: &str) -> (usize, usize, usize) {
    use unicode_segmentation::UnicodeSegmentation;
    // Count extended grapheme clusters so ZWJ emoji sequences such as 👨‍👩‍👧‍👦
    // are one user-perceived character for length limits.
    let graphemes = text.graphemes(true).collect::<Vec<_>>();
    (
        graphemes.len(),
        graphemes
            .iter()
            .filter(|grapheme| !grapheme.chars().all(char::is_whitespace))
            .count(),
        text.split_whitespace().count(),
    )
}

pub(super) fn percentage_change(
    current: f64,
    previous: f64,
) -> Result<(f64, String), &'static str> {
    if !current.is_finite() || !previous.is_finite() {
        return Err("values must be finite");
    }
    if previous == 0.0 {
        return Err("zero baseline; percentage change is undefined");
    }
    let difference = current - previous;
    let ratio = if difference.is_finite() {
        difference / previous.abs()
    } else {
        // Opposite finite extremes can overflow subtraction while their ratio is small.
        current / previous.abs() - previous.signum()
    };
    let value = ratio * 100.0;
    if !value.is_finite() {
        return Err("percentage change exceeds the finite numeric range");
    }
    let mut text = format!("{value:+.2}");
    if text == "+0.00" || text == "-0.00" {
        text = "0".into();
    } else {
        text = text.trim_end_matches('0').trim_end_matches('.').into();
    }
    Ok((value, format!("{text}%")))
}

#[cfg(test)]
#[path = "output_check_metrics_tests.rs"]
mod tests;
