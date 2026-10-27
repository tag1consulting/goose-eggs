//! `drupal::log_in` with `GOOSE_USER` and `GOOSE_PASS` set. Environment variables
//! are process wide, so this is its own test binary with one test.
mod common;

use common::{check_named, run, transaction, Expect};
use goose_eggs::drupal::{self, Login};
use httpmock::{
    Method::{GET, POST},
    MockServer,
};

const LOGIN_PAGE: &str = include_str!("fixtures/drupal/user-login.html");
const LOGGED_IN_PAGE: &str = include_str!("fixtures/drupal/user-logged-in.html");
const LOGIN_BUILD_ID: &str = "form-XZvc6rdZ5NYQPCeEGEmoHx9TJBA68Wv8KWCfBUSOPKQ";

const BEHAVIOUR: &str = "log_in uses GOOSE_USER and GOOSE_PASS over the Login values";

#[tokio::test]
async fn log_in_environment() {
    std::env::set_var("GOOSE_USER", "admin");
    std::env::set_var("GOOSE_PASS", "secret");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/user/login");
        then.status(200).body(LOGIN_PAGE);
    });
    let post = server.mock(|when, then| {
        when.method(POST)
            .path("/user/login")
            .x_www_form_urlencoded_tuple("name", "admin")
            .x_www_form_urlencoded_tuple("pass", "secret")
            .x_www_form_urlencoded_tuple("form_build_id", LOGIN_BUILD_ID)
            .x_www_form_urlencoded_tuple("form_id", "user_login_form");
        then.status(303).header("location", "/user/1");
    });
    // The page title names the user that logged in, `admin`, not the Login value.
    server.mock(|when, then| {
        when.method(GET).path("/user/1");
        then.status(200).body(LOGGED_IN_PAGE);
    });

    let metrics = run(
        &server.base_url(),
        vec![transaction(BEHAVIOUR, |user| {
            Box::pin(async move {
                let login = Login::builder()
                    .username("someone")
                    .password("other")
                    .build();
                drupal::log_in(user, &login).await?;
                Ok(())
            })
        })],
    )
    .await;

    let mut problems: Vec<String> = check_named(&metrics, BEHAVIOUR, &["user/login"], Expect::Pass)
        .into_iter()
        .collect();
    if post.hits() != 1 {
        problems.push(format!(
            "behaviour {}: form posted with the environment's username and password {} times, expected 1",
            BEHAVIOUR,
            post.hits()
        ));
    }
    common::assert_all(problems);
}
