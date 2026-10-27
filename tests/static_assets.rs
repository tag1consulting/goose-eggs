//! Static asset loading, and `validate_page` against
//! `validate_and_load_static_assets`, in one Goose run against one mock server.
//! Each behaviour loads a page whose assets live under its own prefix, so the
//! mock's hit counts say which behaviour requested what.
mod common;

use common::{check, run, transaction, Expect};
use goose::prelude::*;
use goose_eggs::{load_static_elements, validate_and_load_static_assets, validate_page, Validate};
use httpmock::{Method::GET, Mock, MockServer};

// (behaviour, asset prefix, whether its assets must be loaded)
const LOAD: (&str, &str, bool) = ("load_static_elements loads every local asset", "lse", true);
const VALIDATE_AND_LOAD: (&str, &str, bool) = (
    "validate_and_load_static_assets loads assets after a pass",
    "vla",
    true,
);
const VALIDATE_ONLY: (&str, &str, bool) = ("validate_page loads no assets", "vp", false);
const VALIDATE_AND_LOAD_FAILED: (&str, &str, bool) = (
    "validate_and_load_static_assets loads no assets after a failure",
    "vlf",
    false,
);
const NO_RESPONSE: &str = "validate_page fails without a response";

// A page whose assets are all under `/{prefix}/`: four local ones, written in
// four ways, plus one on another host and one link that is not an asset.
fn page(base: &str, other: &str, port: u16, prefix: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <title>Assets {prefix}</title>
  <link href="{base}/{prefix}/abs.css" rel="stylesheet" />
  <link href="{prefix}/rel.css?v=1" rel="stylesheet" />
  <script src="/{prefix}/root.js?a=1&amp;b=2"></script>
</head>
<body>
  <p>Body of {prefix}</p>
  <img src="//127.0.0.1:{port}/{prefix}/proto.png" />
  <img src="{other}/{prefix}/other.png" />
  <a href="/{prefix}/page.html">not an asset</a>
</body>
</html>
"#,
        base = base,
        other = other,
        port = port,
        prefix = prefix
    )
}

struct Assets<'a> {
    local: Vec<(&'static str, Mock<'a>)>,
    other: Mock<'a>,
    link: Mock<'a>,
}

fn mock_page<'a>(server: &'a MockServer, other: &'a MockServer, prefix: &str) -> Assets<'a> {
    let other_base = format!("http://localhost:{}", other.port());
    let body = page(&server.base_url(), &other_base, server.port(), prefix);
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}/page", prefix));
        then.status(200).body(body);
    });
    let asset = |name: &'static str| {
        let mock = server.mock(|when, then| {
            let when = when.method(GET).path(format!("/{}/{}", prefix, name));
            if name == "root.js" {
                // Decoded from `a=1&amp;b=2`.
                when.query_param("a", "1").query_param("b", "2");
            }
            then.status(200).body("asset");
        });
        (name, mock)
    };
    Assets {
        local: vec![
            asset("abs.css"),
            asset("rel.css"),
            asset("root.js"),
            asset("proto.png"),
        ],
        other: other.mock(|when, then| {
            when.method(GET).path(format!("/{}/other.png", prefix));
            then.status(200).body("asset");
        }),
        link: server.mock(|when, then| {
            when.method(GET).path(format!("/{}/page.html", prefix));
            then.status(200).body("page");
        }),
    }
}

// Hit count problems for one behaviour's assets.
fn check_assets(behaviour: &str, assets: &Assets, loaded: bool) -> Vec<String> {
    let expected = usize::from(loaded);
    let mut problems = Vec::new();
    for (name, mock) in &assets.local {
        if mock.hits() != expected {
            problems.push(format!(
                "behaviour {}: asset {} requested {} times, expected {}",
                behaviour,
                name,
                mock.hits(),
                expected
            ));
        }
    }
    for (name, mock) in [("other host", &assets.other), ("page link", &assets.link)] {
        if mock.hits() != 0 {
            problems.push(format!(
                "behaviour {}: {} requested {} times, expected 0",
                behaviour,
                name,
                mock.hits()
            ));
        }
    }
    problems
}

// The page path and body marker of a behaviour's prefix.
fn paths(prefix: &'static str) -> (&'static str, &'static str) {
    match prefix {
        "lse" => ("/lse/page", "Body of lse"),
        "vla" => ("/vla/page", "Body of vla"),
        "vp" => ("/vp/page", "Body of vp"),
        _ => ("/vlf/page", "Body of vlf"),
    }
}

#[tokio::test]
async fn static_assets() {
    let server = MockServer::start();
    let other = MockServer::start();
    let load = mock_page(&server, &other, LOAD.1);
    let validate_and_load = mock_page(&server, &other, VALIDATE_AND_LOAD.1);
    let validate_only = mock_page(&server, &other, VALIDATE_ONLY.1);
    let validate_and_load_failed = mock_page(&server, &other, VALIDATE_AND_LOAD_FAILED.1);

    let transactions = vec![
        transaction(LOAD.0, |user| {
            Box::pin(async move {
                let (path, _) = paths(LOAD.1);
                let goose = user.get_named(path, LOAD.0).await?;
                let html = goose.response.unwrap().text().await.unwrap();
                load_static_elements(user, &html).await;
                Ok(())
            })
        }),
        transaction(VALIDATE_AND_LOAD.0, |user| {
            Box::pin(async move {
                let (path, marker) = paths(VALIDATE_AND_LOAD.1);
                let goose = user.get_named(path, VALIDATE_AND_LOAD.0).await?;
                let validate = Validate::builder().title("Assets").build();
                let html = validate_and_load_static_assets(user, goose, &validate).await?;
                returned_body(&html, marker)
            })
        }),
        transaction(VALIDATE_ONLY.0, |user| {
            Box::pin(async move {
                let (path, marker) = paths(VALIDATE_ONLY.1);
                let goose = user.get_named(path, VALIDATE_ONLY.0).await?;
                let validate = Validate::builder().title("Assets").build();
                let html = validate_page(user, goose, &validate).await?;
                returned_body(&html, marker)
            })
        }),
        transaction(VALIDATE_AND_LOAD_FAILED.0, |user| {
            Box::pin(async move {
                let (path, _) = paths(VALIDATE_AND_LOAD_FAILED.1);
                let goose = user.get_named(path, VALIDATE_AND_LOAD_FAILED.0).await?;
                let validate = Validate::builder().title("Missing").build();
                validate_and_load_static_assets(user, goose, &validate).await?;
                Ok(())
            })
        }),
        transaction(NO_RESPONSE, |user| {
            Box::pin(async move {
                // Nothing listens on port 1.
                let goose = user.get_named("http://127.0.0.1:1/", NO_RESPONSE).await?;
                validate_page(user, goose, &Validate::none()).await?;
                Ok(())
            })
        }),
    ];

    let metrics = run(&server.base_url(), transactions).await;

    let mut problems = Vec::new();
    problems.extend(check(&metrics, LOAD.0, Expect::Pass));
    problems.extend(check(&metrics, VALIDATE_AND_LOAD.0, Expect::Pass));
    problems.extend(check(&metrics, VALIDATE_ONLY.0, Expect::Pass));
    problems.extend(check(
        &metrics,
        VALIDATE_AND_LOAD_FAILED.0,
        Expect::Fail("title not found: Missing"),
    ));
    problems.extend(check(
        &metrics,
        NO_RESPONSE,
        // Goose already failed the request, so its own error is the one recorded.
        Expect::Fail("error sending request"),
    ));
    problems.extend(check_assets(LOAD.0, &load, LOAD.2));
    problems.extend(check_assets(
        VALIDATE_AND_LOAD.0,
        &validate_and_load,
        VALIDATE_AND_LOAD.2,
    ));
    problems.extend(check_assets(
        VALIDATE_ONLY.0,
        &validate_only,
        VALIDATE_ONLY.2,
    ));
    problems.extend(check_assets(
        VALIDATE_AND_LOAD_FAILED.0,
        &validate_and_load_failed,
        VALIDATE_AND_LOAD_FAILED.2,
    ));

    // Two behaviours load assets: each loads two css, one js and one image.
    for (name, expected) in [
        ("GET static asset: css", 4),
        ("GET static asset: js", 2),
        ("GET static asset: img", 2),
    ] {
        let count = metrics
            .requests
            .get(name)
            .map_or(0, |r| r.success_count + r.fail_count);
        if count != expected {
            problems.push(format!(
                "static asset request name {}: {} requests, expected {}",
                name, count, expected
            ));
        }
    }
    common::assert_all(problems);
}

// The returned html must be the page body.
fn returned_body(html: &str, marker: &str) -> TransactionResult {
    if html.contains(marker) {
        Ok(())
    } else {
        Err(format!("body not returned: {:?}", html).into())
    }
}
