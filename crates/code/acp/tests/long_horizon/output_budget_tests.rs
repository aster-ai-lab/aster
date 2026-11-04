use super::admitted_output_tokens;
use std::ffi::OsStr;

#[test]
fn output_budget_preserves_original_default_and_requires_an_admitted_cohort() {
    assert_eq!(admitted_output_tokens(None), Ok(4096));
    assert_eq!(admitted_output_tokens(Some(OsStr::new("4096"))), Ok(4096));
    assert_eq!(admitted_output_tokens(Some(OsStr::new("8192"))), Ok(8192));
    for value in ["", "0", "8193", "8192.0", " 8192"] {
        assert!(admitted_output_tokens(Some(OsStr::new(value))).is_err());
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        assert!(admitted_output_tokens(Some(OsStr::from_bytes(b"\xff"))).is_err());
    }
}
