use gumdrop::Options;
use httpmock::{Method::GET, MockServer};

use goose::config::GooseConfiguration;
use goose::goose::get_base_url;
use goose::metrics::GooseCoordinatedOmissionMitigation::Disabled;
use goose::prelude::*;
use goose_eggs::load_static_elements;

#[tokio::test]
// Loads static elements and checks that characters are decoded properly.
async fn test_html_decoding() {
    let html: &str = r#"
        <!DOCTYPE html>
        <head>
          <!-- Check that encoded paths are decoded properly -->
          <script type="text/javascript" src="/test1.js?foo=1&amp;bar=2"></script>
          <!-- Check that decoded paths still work -->
          <script type="text/javascript" src="/test2.js?foo=1&bar=2"></script>
          <title>Title 1234ABCD</title>
        </head>
        <body>
          <p>Test text on the page.</p>
        </body>
        "#;

    let server = MockServer::start();

    let mock_endpoint1 = server.mock(|when, then| {
        when.method(GET)
            .path("/test1.js")
            .query_param("foo", "1")
            .query_param("bar", "2");
        then.status(200).body("test");
    });
    let mock_endpoint2 = server.mock(|when, then| {
        when.method(GET)
            .path("/test2.js")
            .query_param("foo", "1")
            .query_param("bar", "2");
        then.status(200).body("test");
    });

    let config: Vec<&str> = vec![];
    let mut configuration = GooseConfiguration::parse_args_default(&config).unwrap();
    configuration.co_mitigation = Some(Disabled);
    let base_url = get_base_url(Some(server.base_url()), None, None).unwrap();
    let mut user = GooseUser::new(0, "".to_string(), base_url, &configuration, 0, None).unwrap();

    load_static_elements(&mut user, html).await;
    assert_eq!(mock_endpoint1.hits(), 1);
    assert_eq!(mock_endpoint2.hits(), 1);
}
