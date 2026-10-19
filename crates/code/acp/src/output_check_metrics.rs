//! Pure, bounded-output measurements used by the delivery checker.

pub(super) fn text_counts(text: &str) -> (usize, usize, usize) {
    (
        text.chars().count(),
        text.chars().filter(|ch| !ch.is_whitespace()).count(),
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
