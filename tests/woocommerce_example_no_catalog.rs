//! The WooCommerce example when its catalog cannot be found: the run stops at
//! once, every user that started reporting why, instead of running without a
//! catalog. Goose ignores what a `test_start` transaction returns, so the
//! example keeps the failure with the catalog and each user's first
//! transaction reports it. The killswitch this triggers is the process's own,
//! so this test has a file, and a process, of its own.
#[path = "../examples/woocommerce/common.rs"]
mod common;
#[path = "../examples/woocommerce/customer.rs"]
mod customer;
#[path = "../examples/woocommerce/load_test.rs"]
mod load_test;

use goose::config::GooseConfiguration;
use goose::prelude::*;
use httpmock::prelude::*;
use serde_json::Value;

/// The example's load test, set to run for a minute against a shop whose shop
/// page lists no products, so discovering the catalog fails.
#[tokio::test(flavor = "multi_thread")]
async fn no_catalog_stops_the_run() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/shop/");
            then.status(200).body(
                r#"<html><body class="archive woocommerce-shop"><main><p class="woocommerce-info">No products were found matching your selection.</p></main></body></html>"#,
            );
        })
        .await;
    let report = std::env::temp_dir()
        .join(format!(
            "goose-eggs-woocommerce-no-catalog-{}.json",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string();
    let _ = std::fs::remove_file(&report);
    let host = server.base_url();
    let configuration = GooseConfiguration::parse_args_default(&[
        "--users",
        "10",
        "--increase-rate",
        "100",
        "--run-time",
        "60",
        "--host",
        &host,
        "--co-mitigation",
        "disabled",
        "--no-reset-metrics",
        "--no-telnet",
        "--quiet",
        "--no-print-metrics",
        "--report-file",
        &report,
    ])
    .expect("a configuration");
    let attack = GooseAttack::initialize_with_config(configuration).expect("a load test");
    let metrics = load_test::load_test(attack)
        .expect("the example's load test")
        .execute()
        .await
        .expect("the example's load test runs");
    goose::reset_killswitch();
    let report: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap_or_default())
        .unwrap_or_default();

    let mut problems = Vec::new();
    // The run stopped, canceled with discovery's failure as the reason.
    let reason = report["canceled_reason"].as_str().unwrap_or_default();
    if report["ended_by"] != "canceled" || !reason.contains("discovering the catalog failed") {
        problems.push(format!(
            "no catalog case stopped: the run ended by {} with reason {:?}, not canceled for the failed discovery",
            report["ended_by"], reason
        ));
    }
    // Every user that started failed its first transaction with that error.
    let transactions: Vec<_> = metrics.transactions.iter().flatten().collect();
    let ready: Vec<_> = transactions
        .iter()
        .filter(|t| t.transaction_name.name_for_transaction() == "catalog")
        .collect();
    let ready_ok: usize = ready.iter().map(|t| t.success_count).sum();
    let ready_failed: usize = ready.iter().map(|t| t.fail_count).sum();
    if ready_failed == 0 || ready_ok != 0 {
        problems.push(format!(
            "no catalog case first transaction: catalog passed {} times and failed {} times, expected only failures",
            ready_ok, ready_failed
        ));
    }
    let reported = metrics.transaction_errors.values().any(|e| {
        e.transaction_name == "catalog" && e.error.contains("discovering the catalog failed")
    });
    if !reported {
        problems.push(format!(
            "no catalog case reported: no catalog transaction error names the failed discovery: {:?}",
            metrics
                .transaction_errors
                .values()
                .map(|e| (&e.transaction_name, &e.error))
                .collect::<Vec<_>>()
        ));
    }
    // And no other request was made: the users stop at their first transaction.
    if !metrics.requests.is_empty() {
        problems.push(format!(
            "no catalog case ran empty: requests made without a catalog: {:?}",
            metrics.requests.keys().collect::<Vec<_>>()
        ));
    }
    // No transaction ran as if there were a catalog.
    let passed: Vec<String> = transactions
        .iter()
        .filter(|t| t.success_count > 0)
        .map(|t| t.transaction_name.name_for_transaction().to_string())
        .collect();
    if !passed.is_empty() {
        problems.push(format!(
            "no catalog case ran empty: these transactions passed without a catalog: {:?}",
            passed
        ));
    }
    assert!(
        problems.is_empty(),
        "{} no catalog check(s) wrong:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
