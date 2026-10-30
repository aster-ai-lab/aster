#![cfg(test)]

use super::utf16_slice;

#[test]
fn slices_keep_unicode_boundaries_and_empty_insertion_points() {
    for (value, start, end, expected) in [
        ("", 0, 0, Some("")),
        ("A😀中B", 0, 0, Some("")),
        ("A😀中B", 1, 1, Some("")),
        ("A😀中B", 3, 3, Some("")),
        ("A😀中B", 5, 5, Some("")),
        ("A😀中B", 0, 1, Some("A")),
        ("A😀中B", 1, 3, Some("😀")),
        ("A😀中B", 3, 4, Some("中")),
        ("A😀中B", 2, 2, None),
        ("A😀中B", 1, 2, None),
        ("A😀中B", 2, 3, None),
        ("A😀中B", 3, 1, None),
        ("A😀中B", 6, 6, None),
        ("A😀中B", 0, 6, None),
        ("A😀中B", u32::MAX, u32::MAX, None),
    ] {
        assert_eq!(utf16_slice(value, start, end), expected, "{start}..{end}");
    }
}
