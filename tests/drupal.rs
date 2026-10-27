//! `drupal::log_in` and `drupal::search` against pages captured from a Drupal 11
//! Umami site (scripts/capture-fixtures.sh), in one Goose run against one mock
//! server. Each behaviour uses its own path, and each POST mock only answers the
//! exact form fields Drupal expects, so a wrong field shows as a 404.
mod common;

use common::{check_named, run, transaction, Expect};
use goose_eggs::drupal::{self, get_updated_build_id, Login, SearchParams};
use goose_eggs::Validate;
use httpmock::{
    Method::{GET, POST},
    Mock, MockServer,
};

const LOGIN_PAGE: &str = include_str!("fixtures/drupal/user-login.html");
const LOGGED_IN_PAGE: &str = include_str!("fixtures/drupal/user-logged-in.html");
const LOGIN_FAILED_PAGE: &str = include_str!("fixtures/drupal/user-login-failed.html");
const SEARCH_PAGE: &str = include_str!("fixtures/drupal/search.html");
const SEARCH_RESULTS_PAGE: &str = include_str!("fixtures/drupal/search-results.html");
const NODE_EDIT_PAGE: &str = include_str!("fixtures/drupal/node-edit.html");
const NODE_EDIT_AJAX: &str = include_str!("fixtures/drupal/node-edit-ajax.json");

// Form values in the fixtures above; update them when the fixtures are captured again.
const LOGIN_BUILD_ID: &str = "form-XZvc6rdZ5NYQPCeEGEmoHx9TJBA68Wv8KWCfBUSOPKQ";
const SEARCH_BUILD_ID: &str = "form-6JJTfD8JIuUAJ9m832GgsmZkQWTxIvZzVBmOBfStg-w";
const SEARCH_TOKEN: &str = "WGocfXHGkmf9gjSNN-Dn32xH79rXsfXi1PiXfoQ75sc";
const NODE_EDIT_BUILD_ID: &str = "form-xWYk6k-GUvdFV4oJBV2o20cv-eGobXr_6OCYazRY0vA";
const NODE_EDIT_NEW_BUILD_ID: &str = "form-V97tJqqDda6DFSm5PN7QlZIEyKiPLNJbH9mCWBda1r0";

const LOG_IN: &str = "log_in posts the form and validates the logged in page";
const LOG_IN_PATH: &str = "log_in at a custom path";
const LOG_IN_FORM_ID: &str = "log_in posts a custom form_id";
const LOG_IN_DEFAULTS: &str = "log_in defaults to username and password at user/login";
const LOG_IN_STATUS: &str = "log_in with status validation on both pages";
const LOG_IN_PAGE_STATUS: &str = "log_in fails the log in page status";
const LOG_IN_LOGGED_IN_PAGE: &str = "log_in fails the logged in page validation";
const LOG_IN_TITLE: &str = "log_in fails when the logged in title lacks the username";
const LOG_IN_WRONG: &str = "log_in fails on a wrong password";
const LOG_IN_NO_FORM: &str = "log_in fails without a log in form";
const LOG_IN_NO_BUILD_ID: &str = "log_in fails without a form_build_id";
const LOG_IN_NO_FORM_ID: &str = "log_in fails without a form_id";
const SEARCH: &str = "search posts the form and validates both pages";
const SEARCH_VALUES: &str = "search posts extra form values";
const SEARCH_DEFAULTS: &str = "search defaults to Search at search";
const SEARCH_SUBMIT: &str = "search posts a custom submit";
const SEARCH_PAGE_FAILS: &str = "search fails the search page validation";
const SEARCH_RESULTS_FAILS: &str = "search fails the results page validation";

// Serve the log in form at `/{path}` and, for the right fields only, redirect a
// POST there to `/{path}/done`, which serves `landing`. Returns the POST mock.
fn mock_log_in<'a>(
    server: &'a MockServer,
    path: &str,
    page: &str,
    fields: &[(&str, &str)],
    landing: &str,
) -> Mock<'a> {
    let page = page.to_string();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(page);
    });
    let landing = landing.to_string();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}/done", path));
        then.status(200).body(landing);
    });
    server.mock(|when, then| {
        let mut when = when.method(POST).path(format!("/{}", path));
        for (key, value) in fields {
            when = when.x_www_form_urlencoded_tuple(*key, *value);
        }
        then.status(303)
            .header("location", format!("/{}/done", path));
    })
}

// The fields of a log in as `name`:`pass` with the fixture's form.
fn log_in_fields<'a>(name: &'a str, pass: &'a str, form_id: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("name", name),
        ("pass", pass),
        ("form_build_id", LOGIN_BUILD_ID),
        ("form_id", form_id),
        ("op", "Log in"),
    ]
}

// Serve the search form at `/{path}` and the results to a POST of `fields` there.
fn mock_search<'a>(server: &'a MockServer, path: &str, fields: &[(&str, &str)]) -> Mock<'a> {
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(SEARCH_PAGE);
    });
    server.mock(|when, then| {
        let mut when = when.method(POST).path(format!("/{}", path));
        for (key, value) in fields {
            when = when.x_www_form_urlencoded_tuple(*key, *value);
        }
        then.status(200).body(SEARCH_RESULTS_PAGE);
    })
}

#[tokio::test]
async fn drupal_flows() {
    let server = MockServer::start();
    let admin = log_in_fields("admin", "secret", "user_login_form");

    let posts = vec![
        (
            LOG_IN,
            mock_log_in(&server, "en/user/login", LOGIN_PAGE, &admin, LOGGED_IN_PAGE),
            1,
        ),
        (
            LOG_IN_PATH,
            mock_log_in(&server, "custom/login", LOGIN_PAGE, &admin, LOGGED_IN_PAGE),
            1,
        ),
        (
            LOG_IN_FORM_ID,
            mock_log_in(
                &server,
                "formid/login",
                &LOGIN_PAGE.replace(r#"value="user_login_form""#, r#"value="custom_login_form""#),
                &log_in_fields("admin", "secret", "custom_login_form"),
                LOGGED_IN_PAGE,
            ),
            1,
        ),
        (
            LOG_IN_DEFAULTS,
            mock_log_in(
                &server,
                "user/login",
                LOGIN_PAGE,
                &log_in_fields("username", "password", "user_login_form"),
                "<html><head><title>username | Umami</title></head></html>",
            ),
            1,
        ),
        (
            LOG_IN_STATUS,
            mock_log_in(&server, "status/login", LOGIN_PAGE, &admin, LOGGED_IN_PAGE),
            1,
        ),
        (
            LOG_IN_PAGE_STATUS,
            mock_log_in(
                &server,
                "pagestatus/login",
                LOGIN_PAGE,
                &admin,
                LOGGED_IN_PAGE,
            ),
            0,
        ),
        (
            LOG_IN_LOGGED_IN_PAGE,
            mock_log_in(&server, "landing/login", LOGIN_PAGE, &admin, LOGGED_IN_PAGE),
            1,
        ),
        (
            LOG_IN_TITLE,
            mock_log_in(
                &server,
                "title/login",
                LOGIN_PAGE,
                &admin,
                "<html><head><title>Someone else | Umami</title></head></html>",
            ),
            1,
        ),
        (
            LOG_IN_NO_FORM,
            mock_log_in(
                &server,
                "noform/login",
                r#"<html><head><title>Log in</title></head><body><form class="user-login-form"></body></html>"#,
                &admin,
                LOGGED_IN_PAGE,
            ),
            0,
        ),
        (
            LOG_IN_NO_BUILD_ID,
            mock_log_in(
                &server,
                "nobuildid/login",
                &LOGIN_PAGE.replace(r#"name="form_build_id""#, r#"name="form_build""#),
                &admin,
                LOGGED_IN_PAGE,
            ),
            0,
        ),
        (
            LOG_IN_NO_FORM_ID,
            mock_log_in(
                &server,
                "noformid/login",
                &LOGIN_PAGE.replace(r#"name="form_id""#, r#"name="form""#),
                &admin,
                LOGGED_IN_PAGE,
            ),
            0,
        ),
        (
            SEARCH,
            mock_search(
                &server,
                "en/search/node",
                &[
                    ("keys", "mushrooms"),
                    ("op", "Search"),
                    ("form_build_id", SEARCH_BUILD_ID),
                    ("form_id", "search_form"),
                ],
            ),
            1,
        ),
        (
            SEARCH_VALUES,
            mock_search(
                &server,
                "values/search",
                &[
                    ("keys", "mushrooms"),
                    ("form_build_id", SEARCH_BUILD_ID),
                    ("form_id", "search_form"),
                    ("form_token", SEARCH_TOKEN),
                ],
            ),
            1,
        ),
        (
            SEARCH_DEFAULTS,
            mock_search(
                &server,
                "search",
                &[
                    ("keys", ""),
                    ("op", "Search"),
                    ("form_build_id", SEARCH_BUILD_ID),
                    ("form_id", "search_form"),
                ],
            ),
            1,
        ),
        (
            SEARCH_SUBMIT,
            mock_search(&server, "submit/search", &[("op", "Custom Search")]),
            1,
        ),
        (
            SEARCH_PAGE_FAILS,
            mock_search(&server, "failpage/search", &[]),
            0,
        ),
        (
            SEARCH_RESULTS_FAILS,
            mock_search(&server, "failresults/search", &[]),
            1,
        ),
    ];
    // The wrong password is the one log in Drupal answers with the form again.
    server.mock(|when, then| {
        when.method(GET).path("/wrong/login");
        then.status(200).body(LOGIN_PAGE);
    });
    let wrong_post = server.mock(|when, then| {
        when.method(POST)
            .path("/wrong/login")
            .x_www_form_urlencoded_tuple("pass", "wrong");
        then.status(200).body(LOGIN_FAILED_PAGE);
    });

    let transactions = vec![
        transaction(LOG_IN, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("en/user/login")
                    .build();
                let html = drupal::log_in(user, &login).await?;
                returned_body(&html, "admin | Umami Food Magazine")
            })
        }),
        transaction(LOG_IN_PATH, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("custom/login")
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_FORM_ID, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("formid/login")
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_DEFAULTS, |user| {
            Box::pin(async move {
                drupal::log_in(user, &Login::builder().build()).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_STATUS, |user| {
            Box::pin(async move {
                let status = Validate::builder().status(200).build();
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("status/login")
                    .log_in_page_validation(&status)
                    .logged_in_page_validation(&status)
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_PAGE_STATUS, |user| {
            Box::pin(async move {
                let status = Validate::builder().status(201).build();
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("pagestatus/login")
                    .log_in_page_validation(&status)
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_LOGGED_IN_PAGE, |user| {
            Box::pin(async move {
                let landing = Validate::builder().text("not on the page").build();
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("landing/login")
                    .logged_in_page_validation(&landing)
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_TITLE, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("admin")
                    .password("secret")
                    .url("title/login")
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_WRONG, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("admin")
                    .password("wrong")
                    .url("wrong/login")
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_NO_FORM, |user| {
            Box::pin(async move {
                let login = Login::builder().url("noform/login").build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_NO_BUILD_ID, |user| {
            Box::pin(async move {
                let login = Login::builder().url("nobuildid/login").build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(LOG_IN_NO_FORM_ID, |user| {
            Box::pin(async move {
                let login = Login::builder().url("noformid/login").build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        }),
        transaction(SEARCH, |user| {
            Box::pin(async move {
                let search_page = Validate::builder().title("Search").build();
                let results_page = Validate::builder().title("Search for mushrooms").build();
                let params = SearchParams::builder()
                    .keys("mushrooms")
                    .url("en/search/node")
                    .search_page_validation(&search_page)
                    .results_page_validation(&results_page)
                    .build();
                let html = drupal::search(user, &params).await?;
                returned_body(&html, "Search for mushrooms")
            })
        }),
        transaction(SEARCH_VALUES, |user| {
            Box::pin(async move {
                let params = SearchParams::builder()
                    .keys("mushrooms")
                    .url("values/search")
                    .form_values(&["form_build_id", "form_id", "form_token"])
                    .build();
                drupal::search(user, &params).await?;
                Ok(())
            })
        }),
        transaction(SEARCH_DEFAULTS, |user| {
            Box::pin(async move {
                drupal::search(user, &SearchParams::builder().build()).await?;
                Ok(())
            })
        }),
        transaction(SEARCH_SUBMIT, |user| {
            Box::pin(async move {
                let params = SearchParams::builder()
                    .url("submit/search")
                    .submit("Custom Search")
                    .build();
                drupal::search(user, &params).await?;
                Ok(())
            })
        }),
        transaction(SEARCH_PAGE_FAILS, |user| {
            Box::pin(async move {
                let search_page = Validate::builder().title("Missing").build();
                let params = SearchParams::builder()
                    .url("failpage/search")
                    .search_page_validation(&search_page)
                    .build();
                drupal::search(user, &params).await?;
                Ok(())
            })
        }),
        transaction(SEARCH_RESULTS_FAILS, |user| {
            Box::pin(async move {
                let results_page = Validate::builder().title("Missing").build();
                let params = SearchParams::builder()
                    .url("failresults/search")
                    .results_page_validation(&results_page)
                    .build();
                drupal::search(user, &params).await?;
                Ok(())
            })
        }),
    ];

    let metrics = run(&server.base_url(), transactions).await;

    // (behaviour, the requests it makes, expected result)
    let expectations: Vec<(&str, &[&str], Expect)> = vec![
        (LOG_IN, &["en/user/login"], Expect::Pass),
        (LOG_IN_PATH, &["custom/login"], Expect::Pass),
        (LOG_IN_FORM_ID, &["formid/login"], Expect::Pass),
        (LOG_IN_DEFAULTS, &["user/login"], Expect::Pass),
        (LOG_IN_STATUS, &["status/login"], Expect::Pass),
        (
            LOG_IN_PAGE_STATUS,
            &["pagestatus/login"],
            // The status is checked by Goose, so its error is the one recorded.
            Expect::Fail("200 OK: pagestatus/login"),
        ),
        (
            LOG_IN_LOGGED_IN_PAGE,
            &["landing/login"],
            Expect::Fail("text not found on page: not on the page"),
        ),
        (
            LOG_IN_TITLE,
            &["title/login"],
            Expect::Fail("title not found: admin"),
        ),
        (LOG_IN_WRONG, &["wrong/login"], Expect::Fail("login failed")),
        (
            LOG_IN_NO_FORM,
            &["noform/login"],
            Expect::Fail("no user-login-form on page"),
        ),
        (
            LOG_IN_NO_BUILD_ID,
            &["nobuildid/login"],
            Expect::Fail("no form_build_id on page"),
        ),
        (
            LOG_IN_NO_FORM_ID,
            &["noformid/login"],
            Expect::Fail("no form_id on page"),
        ),
        (SEARCH, &["en/search/node"], Expect::Pass),
        (SEARCH_VALUES, &["values/search"], Expect::Pass),
        (SEARCH_DEFAULTS, &["search"], Expect::Pass),
        (SEARCH_SUBMIT, &["submit/search"], Expect::Pass),
        (
            SEARCH_PAGE_FAILS,
            &["failpage/search"],
            Expect::Fail("title not found: Missing"),
        ),
        (
            SEARCH_RESULTS_FAILS,
            &["failresults/search"],
            Expect::Fail("title not found: Missing"),
        ),
    ];
    let mut problems: Vec<String> = expectations
        .into_iter()
        .filter_map(|(behaviour, requests, expect)| {
            check_named(&metrics, behaviour, requests, expect)
        })
        .collect();
    for (behaviour, post, expected) in &posts {
        if post.hits() != *expected {
            problems.push(format!(
                "behaviour {}: form posted with the expected fields {} times, expected {}",
                behaviour,
                post.hits(),
                expected
            ));
        }
    }
    if wrong_post.hits() != 1 {
        problems.push(format!(
            "behaviour {}: wrong password posted {} times, expected 1",
            LOG_IN_WRONG,
            wrong_post.hits()
        ));
    }
    common::assert_all(problems);
}

// The returned html must be the final page.
fn returned_body(html: &str, marker: &str) -> goose::prelude::TransactionResult {
    if html.contains(marker) {
        Ok(())
    } else {
        Err(format!("final page not returned: {:.200}", html).into())
    }
}

#[test]
fn get_updated_build_id_on_drupal_ajax_response() {
    // The edit form's build id, and Drupal's AJAX answer to a change to that form.
    assert!(
        NODE_EDIT_PAGE.contains(NODE_EDIT_BUILD_ID),
        "fixture: the edit form does not carry the build id the AJAX call posted"
    );
    assert_eq!(
        get_updated_build_id(NODE_EDIT_AJAX, NODE_EDIT_BUILD_ID),
        NODE_EDIT_NEW_BUILD_ID,
        "get_updated_build_id case drupal ajax response"
    );
    assert_eq!(
        get_updated_build_id(NODE_EDIT_AJAX, LOGIN_BUILD_ID),
        "none",
        "get_updated_build_id case drupal ajax response, other build id"
    );
}
