#![cfg(test)]

use super::require_database_url;

#[test]
fn an_unset_database_url_names_postgres() {
    let error = require_database_url(None).unwrap_err();
    assert!(error.contains("ASTER_BILLING_DATABASE_URL"));
    assert!(error.contains("Postgres"));
}

#[test]
fn an_empty_database_url_names_postgres() {
    let error = require_database_url(Some("   ")).unwrap_err();
    assert!(error.contains("ASTER_BILLING_DATABASE_URL"));
    assert!(error.contains("Postgres"));
}

#[test]
fn a_postgres_url_is_kept() {
    let url = "postgres://aster_billing:aster_billing@127.0.0.1:5432/aster_billing";
    assert_eq!(require_database_url(Some(url)).unwrap(), url);
}
