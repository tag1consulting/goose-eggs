// Shared by the doc examples, which pull it in with a hidden
// `# mod support { include!("../tests/doc/support.rs"); }` line, so each example
// can check its result against a real response in a line or two. It is not a
// test target of its own.
use goose::config::GooseConfiguration;
use goose::goose::get_base_url;
use goose::prelude::*;
use goose_eggs::drupal::{Login, SearchParams};
use goose_eggs::Validate;
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use std::future::Future;

const LOGIN_PAGE: &str = include_str!("../fixtures/drupal/user-login.html");
const SEARCH_PAGE: &str = include_str!("../fixtures/drupal/search.html");
const SEARCH_RESULTS_PAGE: &str = include_str!("../fixtures/drupal/search-results.html");
const LOGIN_BUILD_ID: &str = "form-XZvc6rdZ5NYQPCeEGEmoHx9TJBA68Wv8KWCfBUSOPKQ";
const SEARCH_BUILD_ID: &str = "form-6JJTfD8JIuUAJ9m832GgsmZkQWTxIvZzVBmOBfStg-w";
const SEARCH_TOKEN: &str = "WGocfXHGkmf9gjSNN-Dn32xH79rXsfXi1PiXfoQ75sc";

/// A response for `/`: status, headers and body.
pub struct Page<'a> {
    pub status: u16,
    pub headers: &'a [(&'a str, &'a str)],
    pub body: &'a str,
}

/// A 200 page with this body and no extra headers.
pub fn page(body: &str) -> Page<'_> {
    Page {
        status: 200,
        headers: &[],
        body,
    }
}

/// A server answering every path without a dot with `page`, except `/landing`,
/// which answers 200 so a redirect has somewhere to land. Paths with a dot are
/// left for an example to serve its own assets.
pub fn server(page: &Page) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/landing");
        then.status(200).body("landing");
    });
    server.mock(|when, then| {
        when.method(GET)
            .path_matches(regex::Regex::new(r"^/[^.]*$").unwrap());
        let mut then = then.status(page.status).body(page.body);
        for (name, value) in page.headers {
            then = then.header(*name, *value);
        }
    });
    server
}

/// A user whose host is `server`.
pub fn user(server: &MockServer) -> GooseUser {
    let configuration = GooseConfiguration::parse_args_default(&[] as &[&str]).unwrap();
    let base_url = get_base_url(Some(server.base_url()), None, None).unwrap();
    GooseUser::new(0, "".into(), base_url, &configuration, 0, None).unwrap()
}

/// Run a future to completion.
pub fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// The failure a transaction recorded, or "no failure".
pub fn failure<T>(result: &Result<T, Box<TransactionError>>) -> String {
    match result {
        Ok(_) => "no failure".to_string(),
        Err(e) => match e.as_ref() {
            TransactionError::RequestFailed { raw_request } => raw_request.error.clone(),
            other => other.to_string(),
        },
    }
}

/// Run `validate` against `page` with `validate_page`.
pub fn check(validate: &Validate, page: &Page) -> Result<String, String> {
    let server = server(page);
    let mut user = user(&server);
    block_on(async {
        let goose = user.get("/").await.unwrap();
        let result = goose_eggs::validate_page(&mut user, goose, validate).await;
        result.map_err(|e| failure::<()>(&Err(e)))
    })
}

/// Panic unless `validate` passes `good` and fails `bad`.
pub fn proves(example: &str, validate: &Validate, good: &Page, bad: &Page) {
    if let Err(e) = check(validate, good) {
        panic!("doc example {}: failed the page it must pass: {}", example, e);
    }
    if check(validate, bad).is_ok() {
        panic!("doc example {}: passed the page it must fail", example);
    }
}

/// A server with Drupal's log in form at `/{path}` that redirects a POST of
/// `username` and `password` to a page titled with the username.
pub fn log_in_server(username: &str, password: &str, path: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(LOGIN_PAGE);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}", path))
            .x_www_form_urlencoded_tuple("name", username)
            .x_www_form_urlencoded_tuple("pass", password)
            .x_www_form_urlencoded_tuple("form_build_id", LOGIN_BUILD_ID)
            .x_www_form_urlencoded_tuple("form_id", "user_login_form");
        then.status(303).header("location", "/logged-in");
    });
    let landing = format!("<html><head><title>{} | Site</title></head></html>", username);
    server.mock(|when, then| {
        when.method(GET).path("/logged-in");
        then.status(200).body(landing);
    });
    server
}

/// Log in with `login` against `server`; panic naming `example` on failure.
pub fn log_in(example: &str, server: &MockServer, login: &Login) {
    let mut user = user(server);
    let result = block_on(goose_eggs::drupal::log_in(&mut user, login));
    if result.is_err() {
        panic!("doc example {}: log in failed: {}", example, failure(&result));
    }
}

/// A server with Drupal's search form at `/{path}`, titled `page_title`, that
/// answers a POST of `fields` with results titled `results_title`.
pub fn search_server(
    path: &str,
    page_title: &str,
    fields: &[(&str, &str)],
    results_title: &str,
) -> MockServer {
    let server = MockServer::start();
    let search_page = SEARCH_PAGE.replace("<title>Search |", &format!("<title>{} |", page_title));
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(search_page);
    });
    let results = SEARCH_RESULTS_PAGE.replace(
        "<title>Search for mushrooms |",
        &format!("<title>{} |", results_title),
    );
    server.mock(|when, then| {
        let mut when = when
            .method(POST)
            .path(format!("/{}", path))
            .x_www_form_urlencoded_tuple("form_build_id", SEARCH_BUILD_ID)
            .x_www_form_urlencoded_tuple("form_id", "search_form");
        for (key, value) in fields {
            when = when.x_www_form_urlencoded_tuple(*key, *value);
        }
        then.status(200).body(results);
    });
    server
}

/// The form_token on the search fixture, for a Drupal 7 style search.
pub fn search_token() -> &'static str {
    SEARCH_TOKEN
}

/// Search with `params` against `server`; panic naming `example` on failure.
pub fn search(example: &str, server: &MockServer, params: &SearchParams) -> String {
    let mut user = user(server);
    let result = block_on(goose_eggs::drupal::search(&mut user, params));
    match result {
        Ok(html) => html,
        Err(_) => panic!("doc example {}: search failed: {}", example, failure(&result)),
    }
}
