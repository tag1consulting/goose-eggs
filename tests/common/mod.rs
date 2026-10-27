//! Shared setup for the integration tests: each test file starts one mock server
//! and runs one Goose attack whose scenario has one transaction per behaviour,
//! then checks every behaviour against the run's metrics.
#![allow(dead_code)]

use goose::config::GooseConfiguration;
use goose::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// What a behaviour's transaction must do in the run.
#[derive(Clone, Copy, Debug)]
pub enum Expect {
    /// The transaction succeeds.
    Pass,
    /// The transaction fails, and a request error contains this text.
    Fail(&'static str),
}

/// One user running every transaction once. With --iterations Goose never resets
/// the metrics, so every request and transaction is counted.
pub fn configuration(host: &str) -> GooseConfiguration {
    GooseConfiguration::parse_args_default(&[
        "--users",
        "1",
        "--increase-rate",
        "1",
        "--iterations",
        "1",
        "--host",
        host,
        "--co-mitigation",
        "disabled",
        "--no-telnet",
        "--no-websocket",
        "--quiet",
    ])
    .expect("failed to parse options and generate a configuration")
}

/// A transaction named `name` running `f`.
pub fn transaction<F>(name: &'static str, f: F) -> Transaction
where
    F: for<'r> Fn(
            &'r mut GooseUser,
        ) -> Pin<Box<dyn Future<Output = TransactionResult> + Send + 'r>>
        + Send
        + Sync
        + 'static,
{
    Transaction::new(Arc::new(f)).set_name(name)
}

/// Run every transaction once, in order, and return the metrics.
pub async fn run(host: &str, transactions: Vec<Transaction>) -> GooseMetrics {
    let mut scenario = scenario!("Behaviours");
    for transaction in transactions {
        scenario = scenario.register_transaction(transaction);
    }
    GooseAttack::initialize_with_config(configuration(host))
        .unwrap()
        .register_scenario(scenario)
        .execute()
        .await
        .unwrap()
}

/// (successes, failures) of the transaction named `name`.
pub fn transaction_counts(metrics: &GooseMetrics, name: &str) -> Option<(usize, usize)> {
    metrics
        .transactions
        .iter()
        .flatten()
        .find(|t| t.transaction_name.name_for_transaction() == name)
        .map(|t| (t.success_count, t.fail_count))
}

/// Every request error recorded for requests named `name`.
pub fn request_errors(metrics: &GooseMetrics, name: &str) -> Vec<String> {
    metrics
        .errors
        .values()
        .filter(|e| e.name == name)
        .map(|e| e.error.clone())
        .collect()
}

/// Check one behaviour, returning a message naming it when it is wrong. The
/// behaviour's transaction and its requests share its name.
pub fn check(metrics: &GooseMetrics, behaviour: &str, expect: Expect) -> Option<String> {
    check_named(metrics, behaviour, &[behaviour], expect)
}

/// Check one behaviour whose requests are named `requests`.
pub fn check_named(
    metrics: &GooseMetrics,
    behaviour: &str,
    requests: &[&str],
    expect: Expect,
) -> Option<String> {
    let errors: Vec<String> = requests
        .iter()
        .flat_map(|name| request_errors(metrics, name))
        .collect();
    let counts = transaction_counts(metrics, behaviour);
    match (expect, counts) {
        (_, None) => Some(format!("behaviour {}: transaction did not run", behaviour)),
        (Expect::Pass, Some((1, 0))) if errors.is_empty() => None,
        (Expect::Pass, Some((ok, failed))) => Some(format!(
            "behaviour {}: expected a pass, got {} passed, {} failed, errors {:?}",
            behaviour, ok, failed, errors
        )),
        (Expect::Fail(text), Some((0, 1))) if errors.iter().any(|e| e.contains(text)) => None,
        (Expect::Fail(text), Some((ok, failed))) => Some(format!(
            "behaviour {}: expected a failure with {:?}, got {} passed, {} failed, errors {:?}",
            behaviour, text, ok, failed, errors
        )),
    }
}

/// Panic listing every wrong behaviour at once, so each break names all it broke.
pub fn assert_all(problems: Vec<String>) {
    assert!(
        problems.is_empty(),
        "{} behaviour(s) wrong:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
