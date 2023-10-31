use gumdrop::Options;
use httpmock::{Method::GET, MockServer};

use goose::config::GooseConfiguration;
use goose::prelude::*;

// Paths used in load tests performed during these tests.
const PATH: &str = "/one";

const HTML: &str = r#"
<!DOCTYPE html>
<head>
  <title>Title 1234ABCD</title>
</head>
<body>
  <p>Test text on the page.</p>
</body>
"#;

// Test transaction.
pub async fn get_path_valid(user: &mut GooseUser) -> TransactionResult {
    let goose = user.get(PATH).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder()
            .title("1234ABCD")
            .not_title("Example")
            .text("Test text")
            .text("<!DOCTYPE html>")
            .not_text("<!DocType html>")
            .header_value("foo", "bar")
            .not_header("bar")
            .build(),
    )
    .await?;

    Ok(())
}

// Build appropriate configuration for these tests.
fn build_configuration(server: &MockServer) -> GooseConfiguration {
    // Declare server_url so its lifetime is sufficient when needed.
    let server_url = server.base_url();

    // Common elements in all our tests.
    let configuration = vec![
        "--users",
        "1",
        "--hatch-rate",
        "4",
        "--iterations",
        "1",
        "--host",
        &server_url,
        "--co-mitigation",
        "disabled",
        "--quiet",
    ];

    // Parse these options to generate a GooseConfiguration.
    GooseConfiguration::parse_args_default(&configuration)
        .expect("failed to parse options and generate a configuration")
}

async fn run_load_test(server: &MockServer) -> GooseMetrics {
    // Run the Goose Attack.
    let goose_metrics = build_load_test(
        build_configuration(server),
        vec![scenario!("LoadTest").register_transaction(transaction!(get_path_valid))],
        None,
        None,
    )
    .execute()
    .await
    .unwrap();

    // Load test always launches 1 user and makes 1 request.
    assert!(goose_metrics.total_users == 1);
    // Provide debug if this fails.
    if goose_metrics.requests.len() != 1 {
        println!("EXPECTED ONE REQUEST: {:#?}", goose_metrics.requests);
    }
    assert!(goose_metrics.requests.len() == 1);

    goose_metrics
}

// Create a GooseAttack object from the configuration, Scenarios, and optional start and
// stop Transactions.
#[allow(dead_code)]
pub fn build_load_test(
    configuration: GooseConfiguration,
    scenarios: Vec<Scenario>,
    start_transaction: Option<&Transaction>,
    stop_transaction: Option<&Transaction>,
) -> GooseAttack {
    // First set up the common base configuration.
    let mut goose = crate::GooseAttack::initialize_with_config(configuration).unwrap();

    for scenario in scenarios {
        goose = goose.register_scenario(scenario.clone());
    }

    if let Some(transaction) = start_transaction {
        goose = goose.test_start(transaction.clone());
    }

    if let Some(transaction) = stop_transaction {
        goose = goose.test_stop(transaction.clone());
    }

    goose
}

#[tokio::test]
// Make a single request and validate everything.
async fn test_valid() {
    // Start the mock server.
    let server = MockServer::start();

    let mock_endpoint =
        // Set up PATH, store in vector at KEY_ONE.
        server.mock(|when, then| {
            when.method(GET).path(PATH);
            then.status(200)
                .header("foo", "bar")
                .body(HTML);
        });

    let goose_metrics = run_load_test(&server).await;
    assert!(mock_endpoint.hits() == 1);

    // Provide debug if this fails.
    if !goose_metrics.errors.is_empty() {
        println!("UNEXPECTED ERRORS: {:#?}", goose_metrics.errors);
    }
    assert!(goose_metrics.errors.is_empty());
}

#[tokio::test]
// Make a single request and confirm detection of invalid status code.
async fn test_invalid_status() {
    // Start the mock server.
    let server = MockServer::start();

    let mock_endpoint =
        // Set up PATH, store in vector at KEY_ONE.
        server.mock(|when, then| {
            when.method(GET).path(PATH);
            then.status(404)
                .header("foo", "bar")
                .body(HTML);
        });

    let goose_metrics = run_load_test(&server).await;
    assert!(mock_endpoint.hits() == 1);

    // Provide debug if this fails.
    if goose_metrics.errors.len() != 1 {
        println!("EXPECTED ONE ERRORS: {:#?}", goose_metrics.errors);
    }
    assert!(goose_metrics.errors.len() == 1);
}

#[tokio::test]
// Make a single request and confirm detection of invalid header.
async fn test_invalid_header() {
    // Start the mock server.
    let server = MockServer::start();

    let mock_endpoint =
        // Set up PATH, store in vector at KEY_ONE.
        server.mock(|when, then| {
            when.method(GET).path(PATH);
            then.status(200)
                .header("bar", "foo")
                .body(HTML);
        });

    let goose_metrics = run_load_test(&server).await;
    assert!(mock_endpoint.hits() == 1);

    // Provide debug if this fails.
    if goose_metrics.errors.len() != 1 {
        println!("EXPECTED ONE ERRORS: {:#?}", goose_metrics.errors);
    }
    assert!(goose_metrics.errors.len() == 1);
}

#[tokio::test]
// Make a single request and confirm detection of invalid header value.
async fn test_invalid_header_value() {
    // Start the mock server.
    let server = MockServer::start();

    let mock_endpoint =
        // Set up PATH, store in vector at KEY_ONE.
        server.mock(|when, then| {
            when.method(GET).path(PATH);
            then.status(200)
                .header("foo", "invalid")
                .body(HTML);
        });

    let goose_metrics = run_load_test(&server).await;
    assert!(mock_endpoint.hits() == 1);

    // Provide debug if this fails.
    if goose_metrics.errors.len() != 1 {
        println!("EXPECTED ONE ERRORS: {:#?}", goose_metrics.errors);
    }
    assert!(goose_metrics.errors.len() == 1);
}
