#![cfg(test)]

use super::*;

#[test]
fn text_measurement_counts_unicode_and_punctuation_without_guessing_words() {
    assert_eq!(text_counts("报告，\n2 秒🚀"), (8, 6, 3));
    assert_eq!(text_counts(&"字".repeat(301)), (301, 301, 1));
    assert_eq!(text_counts(""), (0, 0, 0));
}

#[test]
fn percentage_output_uses_ascii_signs_and_omits_insignificant_zeros() {
    for (current, previous, expected) in [
        (110.0, 100.0, "+10%"),
        (90.0, 100.0, "-10%"),
        (150.0, 100.0, "+50%"),
        (75.0, 150.0, "-50%"),
        (102.5, 100.0, "+2.5%"),
        (100.0, 100.0, "0%"),
        (99.999, 100.0, "0%"),
        (-90.0, -100.0, "+10%"),
        (f64::MAX, -f64::MAX, "+200%"),
        (-f64::MAX, f64::MAX, "-200%"),
    ] {
        assert_eq!(percentage_change(current, previous).unwrap().1, expected);
    }
    assert!((percentage_change(1.0, 3.0).unwrap().0 + 66.66666666666666).abs() < 1e-10);
}

#[test]
fn undefined_and_overflowing_percentages_are_refused() {
    for (current, previous) in [
        (1.0, 0.0),
        (1.0, -0.0),
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (f64::MAX, f64::MIN_POSITIVE),
    ] {
        assert!(percentage_change(current, previous).is_err());
    }
}
