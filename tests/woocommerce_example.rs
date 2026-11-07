//! The WooCommerce example: its choice of account for each returning customer,
//! and when it finds the shop's catalog. Users sharing one account empty each
//! other's carts (checkout answers `409 Conflict`) and drop each other's logins,
//! so each user needs its own. Finding the catalog is a crawl of the shop, made
//! once before the load test, so its requests are not measured and no user
//! waits on it.
#[path = "../examples/woocommerce/common.rs"]
mod common;
#[path = "../examples/woocommerce/customer.rs"]
mod customer;
#[path = "../examples/woocommerce/load_test.rs"]
mod load_test;

use customer::credentials;
use goose::config::GooseConfiguration;
use goose::prelude::*;
use httpmock::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

#[test]
fn credentials_cases() {
    // (case, user index, CUSTOMER_USERNAME, CUSTOMER_PASSWORD, expected)
    let cases = [
        (
            "first user",
            0,
            None,
            None,
            ("customer1", "customer-password"),
        ),
        (
            "eighth user",
            7,
            None,
            None,
            ("customer8", "customer-password"),
        ),
        ("password", 7, None, Some("secret"), ("customer8", "secret")),
        (
            "one account",
            7,
            Some("shopper"),
            None,
            ("shopper", "customer-password"),
        ),
        (
            "one account with password",
            97,
            Some("shopper"),
            Some("secret"),
            ("shopper", "secret"),
        ),
    ];
    for (case, index, username, password, (expected_username, expected_password)) in cases {
        let (got_username, got_password) = credentials(
            index,
            username.map(str::to_string),
            password.map(str::to_string),
        );
        assert_eq!(
            (got_username.as_str(), got_password.as_str()),
            (expected_username, expected_password),
            "credentials case {case}: wrong account for user {index}"
        );
    }

    // At 100 users the returning customers are users 7, 17 and so on to 97;
    // every user, returning customer or not, gets an account of its own.
    let accounts: HashSet<String> = (0..100)
        .map(|index| credentials(index, None, None).0)
        .collect();
    assert_eq!(
        accounts.len(),
        100,
        "credentials case per user: 100 users share {} accounts",
        accounts.len()
    );
}

/// The shop's first two pages, as the demo shop serves them.
const SHOP: &str = include_str!("fixtures/woocommerce/shop-block.html");
const SHOP_PAGE_2: &str = include_str!("fixtures/woocommerce/shop-block-page-2.html");

/// The names `wordpress::discover` and `woocommerce::discover_products` give
/// their requests.
const DISCOVERY: [&str; 2] = ["discover", "discover products"];

/// A shop to discover: its home page, and its shop page with a second page.
/// Every other request (products, cart, checkout, My Account) is answered 404:
/// this test is about when discovery runs, not what the users buy.
async fn shop() -> (MockServer, usize) {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/");
            then.status(200)
                .body(r#"<html><body class="home blog"><main></main></body></html>"#);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/shop/");
            then.status(200).body(SHOP);
        })
        .await;
    let page_2 = server
        .mock_async(|when, then| {
            when.method(GET).path("/shop/page/2/");
            then.status(200).body(SHOP_PAGE_2);
        })
        .await;
    // Only discovery follows the shop's pagination.
    let id = page_2.id;
    (server, id)
}

/// Every line of a JSON log Goose wrote.
fn log_lines(path: &str) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// A request or transaction log entry's field as a number.
fn number(entry: &Value, field: &str) -> u64 {
    entry[field].as_u64().unwrap_or(u64::MAX)
}

/// The example's load test, run for a second by ten users (so all three
/// scenarios run) against a mock shop, with every request and transaction
/// logged and the metrics never reset. Discovery must run once, before the load
/// test: none of its requests in the metrics or the request log, and every
/// user's first transaction starting with the catalog already found, after the
/// last request discovery made.
#[tokio::test(flavor = "multi_thread")]
async fn discovery_before_load_test() {
    let (server, page_2) = shop().await;
    let dir = std::env::temp_dir();
    let requests = dir
        .join(format!(
            "goose-eggs-woocommerce-example-{}-requests.json",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string();
    let transactions = dir
        .join(format!(
            "goose-eggs-woocommerce-example-{}-transactions.json",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string();
    let _ = std::fs::remove_file(&requests);
    let _ = std::fs::remove_file(&transactions);
    let host = server.base_url();
    let configuration = GooseConfiguration::parse_args_default(&[
        "--users",
        "10",
        "--increase-rate",
        "100",
        "--run-time",
        "1",
        "--host",
        &host,
        "--co-mitigation",
        "disabled",
        "--no-reset-metrics",
        "--no-telnet",
        "--quiet",
        "--no-print-metrics",
        "--request-log",
        &requests,
        "--request-format",
        "json",
        "--transaction-log",
        &transactions,
        "--transaction-format",
        "json",
    ])
    .expect("a configuration");
    let attack = GooseAttack::initialize_with_config(configuration).expect("a load test");
    let metrics = load_test::load_test(attack)
        .expect("the example's load test")
        .execute()
        .await
        .expect("the example's load test runs");
    let requests = log_lines(&requests);
    let transactions = log_lines(&transactions);

    // Discovery ran, once: only it follows the shop's pagination.
    let mut problems = Vec::new();
    let page_2 = httpmock::Mock::new(page_2, &server);
    let hits = page_2.hits_async().await;
    if hits != 1 {
        problems.push(format!(
            "discovery case ran once: the shop's second page was loaded {} times, not once",
            hits
        ));
    }
    page_2.delete_async().await;

    // None of its requests are in the metrics.
    let measured: Vec<&String> = metrics
        .requests
        .values()
        .filter(|r| DISCOVERY.contains(&r.path.as_str()))
        .map(|r| &r.path)
        .collect();
    if !measured.is_empty() {
        problems.push(format!(
            "discovery case metrics: the measured run counts discovery's requests {:?}",
            measured
        ));
    }

    // Nor in the request log.
    let logged: Vec<&Value> = requests
        .iter()
        .filter(|r| DISCOVERY.iter().any(|name| r["name"] == *name))
        .collect();
    if !logged.is_empty() {
        problems.push(format!(
            "discovery case request log: {} of discovery's requests are logged, the first {}",
            logged.len(),
            logged.first().map(|r| r.to_string()).unwrap_or_default()
        ));
    }

    // Every user's first transaction starts with the catalog found: after the
    // last request discovery made, if any were made during the run.
    let mut first: BTreeMap<u64, u64> = BTreeMap::new();
    for transaction in &transactions {
        let start = first.entry(number(transaction, "user")).or_insert(u64::MAX);
        *start = (*start).min(number(transaction, "elapsed"));
    }
    if first.len() != 10 {
        problems.push(format!(
            "discovery case users: {} of 10 users ran a transaction",
            first.len()
        ));
    }
    let last_discovery = logged.iter().map(|r| number(r, "elapsed")).max();
    if let Some(last) = last_discovery {
        let waited: Vec<String> = first
            .iter()
            .filter(|(_, start)| **start <= last)
            .map(|(user, start)| format!("user {} at {} ms", user, start))
            .collect();
        if !waited.is_empty() {
            problems.push(format!("discovery case first transaction: these users started before discovery's last request at {} ms, so waited on it: {}", last, waited.join(", ")));
        }
    }
    assert!(
        problems.is_empty(),
        "{} discovery check(s) wrong:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
