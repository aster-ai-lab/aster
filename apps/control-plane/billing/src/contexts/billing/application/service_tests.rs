#![cfg(test)]
use super::*;
use crate::contexts::billing::domain::LineKind;

const STARTED: i64 = 1_700_000_000;

#[test]
fn parse_names_the_closed_set() {
    assert_eq!(
        BillingCode::parse("conflict").unwrap(),
        BillingCode::Conflict
    );
    assert_eq!(
        BillingCode::parse("store_unavailable").unwrap(),
        BillingCode::StoreUnavailable
    );
    let err = BillingCode::parse("nope").unwrap_err();
    assert!(err.contains("invalid_request"));
    assert!(err.contains("store_unavailable"));
    assert!(err.contains("nope"));
}

#[tokio::test]
async fn an_incomplete_plan_stays_and_new_subscriptions_use_the_next_version() {
    let billing = crate::open_memory().await.unwrap();
    billing
        .create_metric(
            crate::client::AGENT_SEARCH_METRIC,
            crate::client::AGENT_SEARCH_FIELD,
        )
        .await
        .unwrap();
    billing
        .create_metric(
            crate::client::AGENT_PARSE_METRIC,
            crate::client::AGENT_PARSE_FIELD,
        )
        .await
        .unwrap();
    billing
        .create_plan(
            crate::client::AGENT_SERVICES_PLAN,
            "USD",
            0,
            vec![Charge {
                metric_code: crate::client::AGENT_SEARCH_METRIC.to_owned(),
                unit_amount: 7,
            }],
        )
        .await
        .unwrap();
    let old_org = "11111111-1111-1111-1111-111111111111";
    billing.create_customer(old_org, old_org).await.unwrap();
    let old = crate::client::agent_services_subscription_id(old_org);
    billing
        .create_subscription(&old, old_org, crate::client::AGENT_SERVICES_PLAN, STARTED)
        .await
        .unwrap();
    assert_eq!(billing.ensure_agent_services(old_org).await.unwrap(), old);
    let witness_org = "33333333-3333-3333-3333-333333333333";
    billing
        .create_customer(witness_org, witness_org)
        .await
        .unwrap();
    let witness = "agent:witness";
    billing
        .create_subscription(
            witness,
            witness_org,
            crate::client::AGENT_SERVICES_PLAN,
            STARTED,
        )
        .await
        .unwrap();
    let witness_invoice = billing.bill(witness, STARTED + 10).await.unwrap();
    let witness_search = witness_invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_SEARCH_METRIC)
        .unwrap();
    assert_eq!(witness_search.unit_amount, 7);
    assert!(witness_invoice
        .lines
        .iter()
        .all(|line| line.code != crate::client::AGENT_PARSE_METRIC));
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(crate::client::AGENT_SEARCH_FIELD.to_owned(), 1);
    billing
        .ingest(IngestCommand {
            transaction_id: "old-search".into(),
            external_subscription_id: old.clone(),
            code: crate::client::AGENT_SEARCH_METRIC.into(),
            timestamp: Some(STARTED),
            properties,
        })
        .await
        .unwrap();
    let new_org = "22222222-2222-2222-2222-222222222222";
    let new = billing.ensure_agent_services(new_org).await.unwrap();
    assert_eq!(billing.ensure_agent_services(old_org).await.unwrap(), old);
    let old_invoice = billing.bill(&old, STARTED + 10).await.unwrap();
    let search = old_invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_SEARCH_METRIC)
        .unwrap();
    assert_eq!(search.unit_amount, 7);
    let old_pages = old_invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_PARSE_METRIC)
        .unwrap();
    assert_eq!(old_pages.unit_amount, 1);
    assert_eq!(old_pages.quantity, 0);
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(crate::client::AGENT_PARSE_FIELD.to_owned(), 2);
    billing
        .ingest(IngestCommand {
            transaction_id: "new-parse".into(),
            external_subscription_id: new.clone(),
            code: crate::client::AGENT_PARSE_METRIC.into(),
            timestamp: Some(STARTED),
            properties,
        })
        .await
        .unwrap();
    let new_invoice = billing.bill(&new, STARTED + 10).await.unwrap();
    let search = new_invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_SEARCH_METRIC)
        .unwrap();
    assert_eq!(search.unit_amount, 7);
    let pages = new_invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_PARSE_METRIC)
        .unwrap();
    assert_eq!(pages.unit_amount, 1);
    assert_eq!(pages.quantity, 2);
}

#[tokio::test]
async fn agent_services_bill_parsed_pages() {
    let billing = crate::open_memory().await.unwrap();
    let org = "11111111-1111-1111-1111-111111111111";
    let subscription = billing.ensure_agent_services(org).await.unwrap();
    assert_eq!(
        subscription,
        crate::client::agent_services_subscription_id(org)
    );
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(crate::client::AGENT_PARSE_FIELD.to_owned(), 3);
    billing
        .ingest(IngestCommand {
            transaction_id: "parse-1".into(),
            external_subscription_id: subscription.clone(),
            code: crate::client::AGENT_PARSE_METRIC.into(),
            timestamp: Some(STARTED),
            properties,
        })
        .await
        .unwrap();
    let invoice = billing.bill(&subscription, STARTED).await.unwrap();
    let pages = invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_PARSE_METRIC)
        .expect("parsed page line");
    assert_eq!(pages.quantity, 3);
    assert_eq!(pages.unit_amount, 1);
    assert_eq!(pages.amount, 3);
}

#[tokio::test]
async fn replaying_the_same_transaction_does_not_add_quantity() {
    let billing = crate::open_memory().await.unwrap();
    let org = "11111111-1111-1111-1111-111111111111";
    let subscription = billing.ensure_agent_services(org).await.unwrap();
    let mut properties = std::collections::BTreeMap::new();
    properties.insert(crate::client::AGENT_SEARCH_FIELD.to_owned(), 1);
    let ingest = |properties: std::collections::BTreeMap<String, i64>| IngestCommand {
        transaction_id: "search_request:11111111-1111-1111-1111-111111111111:repeat-me".into(),
        external_subscription_id: subscription.clone(),
        code: crate::client::AGENT_SEARCH_METRIC.into(),
        timestamp: Some(STARTED),
        properties,
    };
    billing.ingest(ingest(properties.clone())).await.unwrap();
    let replay = billing.ingest(ingest(properties)).await.unwrap();
    assert_eq!(
        replay,
        IngestResult::Accepted {
            transaction_id: "search_request:11111111-1111-1111-1111-111111111111:repeat-me".into(),
            duplicate: true,
        }
    );
    let invoice = billing.bill(&subscription, STARTED).await.unwrap();
    let requests = invoice
        .lines
        .iter()
        .find(|line| line.code == crate::client::AGENT_SEARCH_METRIC)
        .expect("search line");
    assert_eq!(requests.quantity, 1);
}

async fn hybrid() -> BillingService {
    let billing = crate::open_memory().await.unwrap();
    billing.create_customer("cust_1", "Acme").await.unwrap();
    billing
        .create_metric("tracked_pages", "pageviews")
        .await
        .unwrap();
    billing
        .create_plan(
            "hybrid",
            "USD",
            500,
            vec![Charge {
                metric_code: "tracked_pages".into(),
                unit_amount: 3,
            }],
        )
        .await
        .unwrap();
    billing
        .create_subscription("sub_1", "cust_1", "hybrid", STARTED)
        .await
        .unwrap();
    billing
}

fn event(transaction_id: &str, code: &str, pageviews: i64, timestamp: i64) -> IngestCommand {
    let mut properties = BTreeMap::new();
    properties.insert("pageviews".into(), pageviews);
    IngestCommand {
        transaction_id: transaction_id.into(),
        external_subscription_id: "sub_1".into(),
        code: code.into(),
        timestamp: Some(timestamp),
        properties,
    }
}

#[tokio::test]
async fn replay_and_unknown_events_do_not_increase_the_bill() {
    let billing = hybrid().await;
    let first = billing
        .ingest(event("tx-1", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    assert!(matches!(
        first,
        IngestResult::Accepted {
            duplicate: false,
            ..
        }
    ));
    let replay = billing
        .ingest(event("tx-1", "tracked_pages", 99, STARTED + 10))
        .await;
    assert_eq!(replay.unwrap_err(), BillingCode::Conflict);
    let identical = billing
        .ingest(event("tx-1", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    assert!(matches!(
        identical,
        IngestResult::Accepted {
            duplicate: true,
            ..
        }
    ));
    let unknown_metric = billing
        .ingest(event("tx-x", "not_a_metric", 50, STARTED + 11))
        .await
        .unwrap();
    assert_eq!(
        unknown_metric,
        IngestResult::Denied {
            error: "unknown_metric".into()
        }
    );
    let mut missing_subscription = event("tx-y", "tracked_pages", 50, STARTED + 12);
    missing_subscription.external_subscription_id = "missing".into();
    let unknown_subscription = billing.ingest(missing_subscription).await.unwrap();
    assert_eq!(
        unknown_subscription,
        IngestResult::Denied {
            error: "unknown_subscription".into()
        }
    );
    billing
        .ingest(event("tx-2", "tracked_pages", 20, STARTED + 20))
        .await
        .unwrap();

    let invoice = billing.bill("sub_1", STARTED + 30).await.unwrap();
    let usage = invoice
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap();
    assert_eq!(usage.quantity, 30);
    assert_eq!(usage.amount, 90);
    let recurring = invoice
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Recurring)
        .unwrap();
    assert_eq!(recurring.amount, 500);
    assert_eq!(invoice.currency, "USD");
    assert_eq!(invoice.total(), 590);
}

#[tokio::test]
async fn a_period_with_no_accepted_usage_has_a_zero_usage_line() {
    let billing = hybrid().await;
    let invoice = billing.bill("sub_1", STARTED + 30).await.unwrap();
    let usage = invoice
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap();
    assert_eq!(usage.quantity, 0);
    assert_eq!(usage.amount, usage.quantity * usage.unit_amount);
    let again = billing.bill("sub_1", STARTED + 40).await.unwrap();
    assert_eq!(again.id, invoice.id);
    assert_eq!(again.lines, invoice.lines);
    println!(
        "zero-usage invoice id={} quantity={} unit={} amount={} rebill={}",
        invoice.id, usage.quantity, usage.unit_amount, usage.amount, again.id
    );
}

#[tokio::test]
async fn billing_the_same_period_returns_the_existing_invoice() {
    let billing = hybrid().await;
    billing
        .ingest(event("tx-1", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    let first = billing.bill("sub_1", STARTED + 30).await.unwrap();
    billing
        .ingest(event("tx-2", "tracked_pages", 20, STARTED + 40))
        .await
        .unwrap();
    let second = billing.bill("sub_1", STARTED + 50).await.unwrap();
    assert_eq!(second.id, first.id);
    let usage = second
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap();
    assert_eq!(usage.quantity, 30);
    assert_eq!(billing.invoice_count().await.unwrap(), 1);
}

#[tokio::test]
async fn two_billing_instances_keep_one_invoice() {
    let path =
        std::env::temp_dir().join(format!("aster-billing-fence-{}.sqlite", uuid::Uuid::new_v4()));
    let first = crate::open_sqlite(&path).await.expect("billing a");
    let second = crate::open_sqlite(&path).await.expect("billing b");
    first.create_customer("cust_1", "Acme").await.unwrap();
    first
        .create_metric("tracked_pages", "pageviews")
        .await
        .unwrap();
    first
        .create_plan(
            "hybrid",
            "USD",
            500,
            vec![Charge {
                metric_code: "tracked_pages".into(),
                unit_amount: 3,
            }],
        )
        .await
        .unwrap();
    first
        .create_subscription("sub_1", "cust_1", "hybrid", STARTED)
        .await
        .unwrap();

    let accepted = first
        .ingest(event("tx-1", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    assert!(matches!(
        accepted,
        IngestResult::Accepted {
            duplicate: false,
            ..
        }
    ));
    let replay = second
        .ingest(event("tx-1", "tracked_pages", 99, STARTED + 10))
        .await;
    assert_eq!(replay.unwrap_err(), BillingCode::Conflict);
    let identical = second
        .ingest(event("tx-1", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    assert!(matches!(
        identical,
        IngestResult::Accepted {
            duplicate: true,
            ..
        }
    ));
    second
        .ingest(event("tx-2", "tracked_pages", 20, STARTED + 20))
        .await
        .unwrap();

    let invoice = first.bill("sub_1", STARTED + 30).await.unwrap();
    let again = second.bill("sub_1", STARTED + 40).await.unwrap();
    assert_eq!(again.id, invoice.id);
    assert_eq!(again.lines, invoice.lines);
    let usage = again
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap();
    assert_eq!(usage.quantity, 30);
    assert_eq!(usage.unit_amount, 3);
    assert_eq!(usage.amount, 90);
    assert_eq!(again.currency, "USD");
    assert_eq!(again.total(), 590);

    first
        .ingest(event("tx-3", "tracked_pages", 5, STARTED + 40))
        .await
        .unwrap();
    let after = second.bill("sub_1", STARTED + 50).await.unwrap();
    assert_eq!(after.id, invoice.id);
    let later = after
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap();
    assert_eq!(later.quantity, 35);
    assert_eq!(first.invoice_count().await.unwrap(), 1);
    assert_eq!(second.invoice_count().await.unwrap(), 1);

    let _ = std::fs::remove_file(&path);
    let mut wal = path.clone().into_os_string();
    wal.push("-wal");
    let _ = std::fs::remove_file(wal);
    let mut shm = path.into_os_string();
    shm.push("-shm");
    let _ = std::fs::remove_file(shm);
}

#[tokio::test]
async fn an_overflowing_usage_amount_is_not_a_successful_charge() {
    let billing = hybrid().await;
    let ingested = billing
        .ingest(event(
            "tx-overflow",
            "tracked_pages",
            i64::MAX,
            STARTED + 10,
        ))
        .await
        .unwrap();
    assert!(matches!(
        ingested,
        IngestResult::Accepted {
            duplicate: false,
            ..
        }
    ));
    let billed = billing.bill("sub_1", STARTED + 30).await;
    assert_eq!(billed.unwrap_err(), BillingCode::InvalidRequest);
    assert_eq!(billing.invoice_count().await.unwrap(), 0);
    println!("OVERFLOW_REFUSED code=invalid_request invoice_count=0");
}

#[tokio::test]
async fn member_usage_ensure_keeps_an_existing_start_and_accepts_tokens() {
    let org = Uuid::new_v4().to_string();
    let billing = crate::open_memory().await.unwrap();
    billing.create_customer(&org, "org").await.unwrap();
    billing
        .create_metric(
            crate::client::MEMBER_USAGE_METRIC,
            crate::client::MEMBER_USAGE_FIELD,
        )
        .await
        .unwrap();
    billing
        .create_plan(
            crate::client::MEMBER_USAGE_PLAN,
            "USD",
            0,
            vec![Charge {
                metric_code: crate::client::MEMBER_USAGE_METRIC.into(),
                unit_amount: 0,
            }],
        )
        .await
        .unwrap();
    let external = crate::client::member_usage_subscription_id(&org);
    billing
        .create_subscription(&external, &org, crate::client::MEMBER_USAGE_PLAN, 100)
        .await
        .unwrap();
    let kept = billing.ensure_member_usage(&org).await.unwrap();
    assert_eq!(kept, external);
    let invoice = billing.bill(&external, 100).await.unwrap();
    assert_eq!(invoice.period_start, 100);
    assert!(invoice.lines.iter().all(|line| line.unit_amount == 0));

    let fresh_org = Uuid::new_v4().to_string();
    let fresh = crate::open_memory().await.unwrap();
    let subscription = fresh.ensure_member_usage(&fresh_org).await.unwrap();
    assert_eq!(
        fresh.ensure_member_usage(&fresh_org).await.unwrap(),
        subscription
    );
    let mut properties = BTreeMap::new();
    properties.insert(crate::client::MEMBER_USAGE_FIELD.into(), 0);
    let accepted = fresh
        .ingest(IngestCommand {
            transaction_id: "member-zero".into(),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(1_700_000_000),
            properties,
        })
        .await
        .unwrap();
    assert!(matches!(
        accepted,
        IngestResult::Accepted {
            duplicate: false,
            ..
        }
    ));
    assert!(fresh.ensure_member_usage("not-a-uuid").await.is_err());
}

async fn prove_member_usage_idempotence(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    assert_eq!(
        billing.ensure_member_usage(&org).await.unwrap(),
        subscription
    );
    let transaction_id = format!("campaign-{}", Uuid::new_v4());
    let mut properties = BTreeMap::new();
    properties.insert(crate::client::MEMBER_USAGE_FIELD.into(), 2);
    let command = IngestCommand {
        transaction_id: transaction_id.clone(),
        external_subscription_id: subscription,
        code: crate::client::MEMBER_USAGE_METRIC.into(),
        timestamp: Some(1_700_000_000),
        properties,
    };
    let first = billing.ingest(command.clone()).await.unwrap();
    assert!(matches!(
        first,
        IngestResult::Accepted {
            duplicate: false,
            ..
        }
    ));
    let second = billing.ingest(command).await.unwrap();
    assert!(matches!(
        second,
        IngestResult::Accepted {
            duplicate: true,
            ..
        }
    ));
}

#[tokio::test]
async fn a_store_outage_is_not_an_invalid_request() {
    let billing = BillingService::new(Arc::new(DownStore));
    assert_eq!(
        billing.ready().await.unwrap_err(),
        BillingCode::StoreUnavailable
    );
    let ingested = billing
        .ingest(IngestCommand {
            transaction_id: "tx".into(),
            external_subscription_id: "sub".into(),
            code: "total_tokens".into(),
            timestamp: Some(1),
            properties: BTreeMap::new(),
        })
        .await;
    assert_eq!(ingested.unwrap_err(), BillingCode::StoreUnavailable);
}

#[tokio::test]
async fn a_lost_insert_rereads_the_stored_fact() {
    let billing = BillingService::new(Arc::new(LostInsertStore::new()));
    let mut properties = BTreeMap::new();
    properties.insert("tokens".into(), 4);
    let result = billing
        .ingest(IngestCommand {
            transaction_id: "tx-race".into(),
            external_subscription_id: "sub".into(),
            code: "total_tokens".into(),
            timestamp: Some(1_700_000_100),
            properties,
        })
        .await;
    assert_eq!(result.unwrap_err(), BillingCode::Conflict);
}

#[tokio::test]
async fn a_later_clock_does_not_change_the_digest() {
    let billing = hybrid().await;
    billing
        .ingest(event("tx-clock", "tracked_pages", 10, STARTED + 10))
        .await
        .unwrap();
    let replay = billing
        .ingest(event("tx-clock", "tracked_pages", 10, STARTED + 80))
        .await
        .unwrap();
    assert!(matches!(
        replay,
        IngestResult::Accepted {
            duplicate: true,
            ..
        }
    ));
}

#[tokio::test]
async fn authority_proofs_match_on_memory_and_postgres() {
    prove_member_usage_idempotence(crate::open_memory().await.unwrap()).await;
    prove_divergent_replay_conflicts(crate::open_memory().await.unwrap()).await;
    prove_clock_is_not_part_of_the_digest(crate::open_memory().await.unwrap()).await;
    prove_later_usage_updates_the_invoice(crate::open_memory().await.unwrap()).await;
    prove_unknown_usage_does_not_increase_the_bill(crate::open_memory().await.unwrap()).await;
    prove_zero_usage_keeps_the_invoice(crate::open_memory().await.unwrap()).await;
    prove_overflow_is_not_a_charge(crate::open_memory().await.unwrap()).await;
    let path = std::env::temp_dir().join(format!("aster-billing-shared-{}.sqlite", Uuid::new_v4()));
    prove_two_connections_keep_one_invoice(
        crate::open_sqlite(&path).await.expect("sqlite a"),
        crate::open_sqlite(&path).await.expect("sqlite b"),
    )
    .await;
    let admin_url = std::env::var("ASTER_BILLING_TEST_POSTGRES_URL")
        .ok()
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| {
            panic!("ASTER_BILLING_TEST_POSTGRES_URL is required for the billing dual-adapter proof")
        });
    // Auto-create a disposable database per run: migration checksums change when
    // the billing schema evolves, making a persistent test database refuse.
    let db_name = format!("aster_billing_test_{}", Uuid::new_v4().simple());
    let admin = aster_orm::PostgresExecutor::connect_no_tls(&admin_url, 2)
        .expect("admin postgres connection");
    let admin_connection = admin.pool().get().await.expect("admin postgres client");
    admin_connection
        .batch_execute(&format!("create database \"{db_name}\""))
        .await
        .expect("disposable billing test database");
    drop(admin_connection);
    drop(admin);
    let test_url = admin_url
        .rsplit_once('/')
        .map(|(prefix, _)| format!("{prefix}/{db_name}"))
        .unwrap_or_else(|| format!("{admin_url}/{db_name}"));
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_member_usage_idempotence(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_divergent_replay_conflicts(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_clock_is_not_part_of_the_digest(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_later_usage_updates_the_invoice(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_unknown_usage_does_not_increase_the_bill(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_zero_usage_keeps_the_invoice(postgres).await;
    let postgres = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_overflow_is_not_a_charge(postgres).await;
    let first = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    let second = crate::open_postgres(&test_url)
        .await
        .expect("postgres billing");
    prove_two_connections_keep_one_invoice(first, second).await;
    // Drop the disposable test database
    if let Ok(cleanup) = aster_orm::PostgresExecutor::connect_no_tls(&admin_url, 2) {
        if let Ok(connection) = cleanup.pool().get().await {
            let _ = connection
                .batch_execute(&format!(
                    "drop database if exists \"{db_name}\" with (force)"
                ))
                .await;
        }
    }
}

async fn prove_divergent_replay_conflicts(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    let transaction_id = format!("conflict-{}", Uuid::new_v4());
    let mut properties = BTreeMap::new();
    properties.insert(crate::client::MEMBER_USAGE_FIELD.into(), 4);
    let mut command = IngestCommand {
        transaction_id: transaction_id.clone(),
        external_subscription_id: subscription,
        code: crate::client::MEMBER_USAGE_METRIC.into(),
        timestamp: Some(1_700_000_100),
        properties,
    };
    billing.ingest(command.clone()).await.unwrap();
    command
        .properties
        .insert(crate::client::MEMBER_USAGE_FIELD.into(), 9);
    assert_eq!(
        billing.ingest(command).await.unwrap_err(),
        BillingCode::Conflict
    );
}

async fn prove_later_usage_updates_the_invoice(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    let stamp = 1_700_000_000_i64;
    let mut first = BTreeMap::new();
    first.insert(crate::client::MEMBER_USAGE_FIELD.into(), 3);
    billing
        .ingest(IngestCommand {
            transaction_id: format!("period-a-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(stamp + 10),
            properties: first,
        })
        .await
        .unwrap();
    let invoice = billing.bill(&subscription, stamp + 20).await.unwrap();
    let mut second = BTreeMap::new();
    second.insert(crate::client::MEMBER_USAGE_FIELD.into(), 5);
    billing
        .ingest(IngestCommand {
            transaction_id: format!("period-b-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(stamp + 30),
            properties: second,
        })
        .await
        .unwrap();
    let again = billing.bill(&subscription, stamp + 40).await.unwrap();
    assert_eq!(again.id, invoice.id);
    let quantity = again
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap()
        .quantity;
    assert_eq!(quantity, 8);
    let stored = billing.bill(&subscription, stamp + 40).await.unwrap();
    assert_eq!(stored.id, again.id);
}

async fn prove_clock_is_not_part_of_the_digest(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    let transaction_id = format!("clock-{}", Uuid::new_v4());
    let mut properties = BTreeMap::new();
    properties.insert(crate::client::MEMBER_USAGE_FIELD.into(), 4);
    let mut command = IngestCommand {
        transaction_id: transaction_id.clone(),
        external_subscription_id: subscription,
        code: crate::client::MEMBER_USAGE_METRIC.into(),
        timestamp: Some(1_700_000_100),
        properties,
    };
    billing.ingest(command.clone()).await.unwrap();
    command.timestamp = Some(1_700_000_180);
    let replay = billing.ingest(command).await.unwrap();
    assert!(matches!(
        replay,
        IngestResult::Accepted {
            duplicate: true,
            ..
        }
    ));
}

async fn prove_unknown_usage_does_not_increase_the_bill(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    let stamp = 1_700_000_000_i64;
    let mut properties = BTreeMap::new();
    properties.insert(crate::client::MEMBER_USAGE_FIELD.into(), 10);
    billing
        .ingest(IngestCommand {
            transaction_id: format!("known-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(stamp + 10),
            properties,
        })
        .await
        .unwrap();
    let unknown = billing
        .ingest(IngestCommand {
            transaction_id: format!("unknown-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: "not_a_metric".into(),
            timestamp: Some(stamp + 11),
            properties: BTreeMap::new(),
        })
        .await
        .unwrap();
    assert!(matches!(unknown, IngestResult::Denied { .. }));
    let invoice = billing.bill(&subscription, stamp + 20).await.unwrap();
    let quantity = invoice
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap()
        .quantity;
    assert_eq!(quantity, 10);
}

async fn prove_zero_usage_keeps_the_invoice(billing: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = billing.ensure_member_usage(&org).await.unwrap();
    let stamp = 1_700_000_000_i64;
    let invoice = billing.bill(&subscription, stamp + 10).await.unwrap();
    let again = billing.bill(&subscription, stamp + 20).await.unwrap();
    assert_eq!(again.id, invoice.id);
    let quantity = again
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap()
        .quantity;
    assert_eq!(quantity, 0);
}

async fn prove_overflow_is_not_a_charge(billing: BillingService) {
    let customer = format!("cust-{}", Uuid::new_v4());
    let plan = format!("plan-{}", Uuid::new_v4());
    let metric = format!("metric-{}", Uuid::new_v4());
    let subscription = format!("sub-{}", Uuid::new_v4());
    billing.create_customer(&customer, "Acme").await.unwrap();
    billing.create_metric(&metric, "units").await.unwrap();
    billing
        .create_plan(
            &plan,
            "USD",
            1,
            vec![Charge {
                metric_code: metric.clone(),
                unit_amount: i64::MAX,
            }],
        )
        .await
        .unwrap();
    billing
        .create_subscription(&subscription, &customer, &plan, 1_700_000_000)
        .await
        .unwrap();
    let mut properties = BTreeMap::new();
    properties.insert("units".into(), 4);
    billing
        .ingest(IngestCommand {
            transaction_id: format!("overflow-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: metric,
            timestamp: Some(1_700_000_010),
            properties,
        })
        .await
        .unwrap();
    assert_eq!(
        billing
            .bill(&subscription, 1_700_000_020)
            .await
            .unwrap_err(),
        BillingCode::InvalidRequest
    );
}

async fn prove_two_connections_keep_one_invoice(first: BillingService, second: BillingService) {
    let org = Uuid::new_v4().to_string();
    let subscription = first.ensure_member_usage(&org).await.unwrap();
    let stamp = 1_700_000_000_i64;
    let mut early = BTreeMap::new();
    early.insert(crate::client::MEMBER_USAGE_FIELD.into(), 10);
    first
        .ingest(IngestCommand {
            transaction_id: format!("conn-a-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(stamp + 10),
            properties: early,
        })
        .await
        .unwrap();
    let mut later = BTreeMap::new();
    later.insert(crate::client::MEMBER_USAGE_FIELD.into(), 20);
    second
        .ingest(IngestCommand {
            transaction_id: format!("conn-b-{}", Uuid::new_v4()),
            external_subscription_id: subscription.clone(),
            code: crate::client::MEMBER_USAGE_METRIC.into(),
            timestamp: Some(stamp + 30),
            properties: later,
        })
        .await
        .unwrap();
    let invoice = first.bill(&subscription, stamp + 20).await.unwrap();
    let again = second.bill(&subscription, stamp + 40).await.unwrap();
    assert_eq!(again.id, invoice.id);
    let quantity = again
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Usage)
        .unwrap()
        .quantity;
    assert_eq!(quantity, 30);
}

struct LostInsertStore {
    planted: std::sync::Mutex<Option<UsageEvent>>,
}

impl LostInsertStore {
    fn new() -> Self {
        Self {
            planted: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl crate::contexts::billing::domain::repository::BillingStore for LostInsertStore {
    async fn put_customer(&self, _: Customer) -> Result<(), String> {
        Ok(())
    }
    async fn customer(&self, _: &str) -> Result<Option<Customer>, String> {
        Ok(None)
    }
    async fn put_metric(&self, _: BillableMetric) -> Result<(), String> {
        Ok(())
    }
    async fn metric(&self, code: &str) -> Result<Option<BillableMetric>, String> {
        Ok(Some(BillableMetric {
            code: code.to_owned(),
            field_name: "tokens".into(),
        }))
    }
    async fn put_plan(&self, _: Plan) -> Result<(), String> {
        Ok(())
    }
    async fn plan(&self, _: &str) -> Result<Option<Plan>, String> {
        Ok(None)
    }
    async fn put_subscription(&self, _: Subscription) -> Result<(), String> {
        Ok(())
    }
    async fn subscription(&self, external_id: &str) -> Result<Option<Subscription>, String> {
        Ok(Some(Subscription {
            external_id: external_id.to_owned(),
            external_customer_id: "cust".into(),
            plan_code: "showback".into(),
            started_at: 1,
        }))
    }
    async fn event(&self, transaction_id: &str) -> Result<Option<UsageEvent>, String> {
        Ok(self
            .planted
            .lock()
            .map_err(|_| "lock".to_owned())?
            .clone()
            .filter(|event| event.transaction_id == transaction_id))
    }
    async fn insert_event(&self, event: UsageEvent) -> Result<bool, String> {
        let mut planted = self.planted.lock().map_err(|_| "lock".to_owned())?;
        if planted.is_none() {
            planted.replace(UsageEvent {
                quantity: event.quantity.saturating_add(1),
                ..event
            });
            return Ok(false);
        }
        Ok(false)
    }
    async fn events_for_subscription(&self, _: &str) -> Result<Vec<UsageEvent>, String> {
        Ok(vec![])
    }
    async fn invoice_for_period(&self, _: &str, _: i64) -> Result<Option<Invoice>, String> {
        Ok(None)
    }
    async fn insert_invoice(&self, _: Invoice) -> Result<bool, String> {
        Ok(true)
    }
    async fn save_invoice(&self, _: Invoice) -> Result<(), String> {
        Ok(())
    }
    async fn invoice_count(&self) -> Result<usize, String> {
        Ok(0)
    }
    async fn ping(&self) -> Result<(), String> {
        Ok(())
    }
}

struct DownStore;

#[async_trait::async_trait]
impl crate::contexts::billing::domain::repository::BillingStore for DownStore {
    async fn put_customer(&self, _: Customer) -> Result<(), String> {
        Err("down".into())
    }
    async fn customer(&self, _: &str) -> Result<Option<Customer>, String> {
        Err("down".into())
    }
    async fn put_metric(&self, _: BillableMetric) -> Result<(), String> {
        Err("down".into())
    }
    async fn metric(&self, _: &str) -> Result<Option<BillableMetric>, String> {
        Err("down".into())
    }
    async fn put_plan(&self, _: Plan) -> Result<(), String> {
        Err("down".into())
    }
    async fn plan(&self, _: &str) -> Result<Option<Plan>, String> {
        Err("down".into())
    }
    async fn put_subscription(&self, _: Subscription) -> Result<(), String> {
        Err("down".into())
    }
    async fn subscription(&self, _: &str) -> Result<Option<Subscription>, String> {
        Err("down".into())
    }
    async fn event(&self, _: &str) -> Result<Option<UsageEvent>, String> {
        Err("down".into())
    }
    async fn insert_event(&self, _: UsageEvent) -> Result<bool, String> {
        Err("down".into())
    }
    async fn events_for_subscription(&self, _: &str) -> Result<Vec<UsageEvent>, String> {
        Err("down".into())
    }
    async fn invoice_for_period(&self, _: &str, _: i64) -> Result<Option<Invoice>, String> {
        Err("down".into())
    }
    async fn insert_invoice(&self, _: Invoice) -> Result<bool, String> {
        Err("down".into())
    }
    async fn save_invoice(&self, _: Invoice) -> Result<(), String> {
        Err("down".into())
    }
    async fn invoice_count(&self) -> Result<usize, String> {
        Err("down".into())
    }
    async fn ping(&self) -> Result<(), String> {
        Err("down".into())
    }
}
