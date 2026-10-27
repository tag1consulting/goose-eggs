//! The `wordpress` flows against pages captured from the theme test data site
//! (examples/wordpress-sites/capture-fixtures.sh), in one Goose run against one
//! mock server. Each behaviour uses its own paths, and each mock only answers
//! the exact fields, headers and nonces WordPress expects, so a wrong one shows
//! as a 404.
mod common;

use common::{check_named, run, transaction, Expect};
use goose::prelude::*;
use goose_eggs::wordpress::{self, Comment, Discover, Login, Post, SearchParams};
use goose_eggs::Validate;
use httpmock::prelude::HttpMockRequest;
use httpmock::{
    Method::{GET, POST},
    Mock, MockServer,
};

const LOGIN_PAGE: &str = include_str!("fixtures/wordpress/login.html");
const LOGIN_FAILED: &str = include_str!("fixtures/wordpress/login-failed.html");
const LOGIN_NO_COOKIE: &str = include_str!("fixtures/wordpress/login-no-test-cookie.html");
const DASHBOARD: &str = include_str!("fixtures/wordpress/dashboard.html");
const LOGGED_OUT: &str = include_str!("fixtures/wordpress/logged-out.html");
const LOGOUT_CONFIRM: &str = include_str!("fixtures/wordpress/logout-confirm.html");
const SEARCH_RESULTS: &str = include_str!("fixtures/wordpress/search-results.html");
const COMMENT_ANONYMOUS: &str = include_str!("fixtures/wordpress/comment-form-anonymous.html");
const COMMENT_LOGGED_IN: &str = include_str!("fixtures/wordpress/comment-form-logged-in.html");
const COMMENT_DUPLICATE: &str = include_str!("fixtures/wordpress/comment-duplicate.html");
const COMMENT_FLOOD: &str = include_str!("fixtures/wordpress/comment-flood.html");
const SITEMAP: &str = include_str!("fixtures/wordpress/sitemap.xml");
const SITEMAP_POSTS: &str = include_str!("fixtures/wordpress/sitemap-posts-post-1.xml");
const REST_POSTS: &str = include_str!("fixtures/wordpress/rest-posts.json");
const REST_POST: &str = include_str!("fixtures/wordpress/rest-post-update.json");
const REST_INVALID_NONCE: &str = include_str!("fixtures/wordpress/rest-invalid-nonce.json");

const LOG_IN: &str = "log_in loads the form for the test cookie, then posts it";
const LOG_IN_PATH: &str = "log_in at a custom path, redirecting to a custom page";
const LOG_IN_WRONG: &str = "log_in fails on WordPress's error box";
const LOG_IN_NO_COOKIE: &str = "log_in fails when WordPress saw no test cookie";
const LOG_IN_NOT_REDIRECTED: &str = "log_in fails when not redirected";
const LOG_IN_NO_FORM: &str = "log_in fails without a log in form";
const LOG_IN_PAGE_STATUS: &str = "log_in fails on the log in page's error status";
const LOG_IN_POST_STATUS: &str = "log_in fails on the post's error status";
const LOG_IN_PAGE_VALIDATION: &str = "log_in fails the log in page validation";
const LOG_IN_LOGGED_IN_VALIDATION: &str = "log_in fails the logged in page validation";
const LOG_OUT: &str = "log_out follows the log out link with its nonce";
const LOG_OUT_STALE: &str = "log_out fails when WordPress asks to confirm";
const LOG_OUT_NO_LINK: &str = "log_out fails without a log out link";
const SEARCH: &str = "search sends encoded keys and the post type";
const SEARCH_DEFAULTS: &str = "search defaults to an empty search on the home page";
const SEARCH_VALIDATION: &str = "search fails the results validation";
const SEARCH_STATUS: &str = "search fails on an error status";
const COMMENT_ANONYMOUS_FIELDS: &str = "post_comment sends the form's fields and random text";
const COMMENT_LOGGED_IN_FIELDS: &str = "post_comment as a logged in user";
const COMMENT_CUSTOM: &str = "post_comment with custom text, parent and handler";
const COMMENT_DUPLICATE_409: &str = "post_comment fails on a duplicate";
const COMMENT_FLOOD_429: &str = "post_comment fails on comment flood protection";
const COMMENT_CLOSED: &str = "post_comment fails without a comment form";
const COMMENT_NOT_ACCEPTED: &str = "post_comment fails when not redirected";
const COMMENT_POST_VALIDATION: &str = "post_comment fails the post page validation";
const COMMENT_COMMENTED_VALIDATION: &str = "post_comment fails the commented page validation";
const CREATE_POST: &str = "create_post sends the nonce and fields";
const UPDATE_POST: &str = "update_post sends the nonce and fields to the post";
const REST_STALE_NONCE: &str = "update_post fails on a stale nonce";
const REST_NO_ID: &str = "create_post fails without a post in the response";
const REST_NOT_JSON: &str = "create_post fails on a response that is not JSON";
const DISCOVER_SITEMAP: &str = "discover reads the sitemap index and its sitemaps";
const DISCOVER_CAP: &str = "discover stops at max_per_kind";
const DISCOVER_REST: &str = "discover falls back to REST, following its pages";
const DISCOVER_REST_CAP: &str = "discover asks REST for no more than max_per_kind";
const DISCOVER_SITEMAP_STATUS: &str = "discover fails on a sitemap's error status";
const DISCOVER_REST_STATUS: &str = "discover fails on a REST error status";

// A transaction error for a wrong result.
fn wrong(message: String) -> Box<TransactionError> {
    Box::new(TransactionError::Custom(message))
}

// The value of a hidden field in a fixture, attributes in any order.
fn fixture_value(html: &str, name: &str) -> String {
    let input = regex::Regex::new(r"(?s)<input\b[^>]*>").unwrap();
    let named = regex::Regex::new(&format!(r#"\bname=["']{}["']"#, regex::escape(name))).unwrap();
    let value = regex::Regex::new(r#"\bvalue=["']([^"']*)["']"#).unwrap();
    let tag = input
        .find_iter(html)
        .map(|m| m.as_str())
        .find(|tag| named.is_match(tag))
        .unwrap();
    value.captures(tag).unwrap()[1].to_string()
}

// The dashboard with its REST nonce and log out nonce replaced.
fn dashboard(rest_nonce: &str, log_out_nonce: &str) -> String {
    let rest = regex::Regex::new(r#"createNonceMiddleware\( "[^"]+""#).unwrap();
    let log_out = regex::Regex::new(r"_wpnonce=[0-9a-f]+").unwrap();
    let html = rest.replace_all(
        DASHBOARD,
        format!(r#"createNonceMiddleware( "{}""#, rest_nonce).as_str(),
    );
    log_out
        .replace_all(&html, format!("_wpnonce={}", log_out_nonce).as_str())
        .to_string()
}

// Serve the log in form at `/{prefix}/wp-login.php` (with WordPress's test
// cookie) and answer a post of `name`:`pass` and the test cookie flag there with
// `answer`. Returns the GET and POST mocks.
fn mock_log_in<'a>(
    server: &'a MockServer,
    prefix: &str,
    page: &str,
    redirect_to: &str,
    answer: (u16, Option<&str>, &str),
) -> (Mock<'a>, Mock<'a>) {
    let path = format!("/{}/wp-login.php", prefix);
    let page = page.to_string();
    let get = server.mock(|when, then| {
        when.method(GET).path(&path);
        then.status(200)
            .header(
                "set-cookie",
                "wordpress_test_cookie=WP%20Cookie%20check; path=/",
            )
            .body(page);
    });
    let (status, location, body) = answer;
    let post = server.mock(|when, then| {
        when.method(POST)
            .path(&path)
            .x_www_form_urlencoded_tuple("log", "admin")
            .x_www_form_urlencoded_tuple("pwd", "secret")
            .x_www_form_urlencoded_tuple("wp-submit", "Log In")
            .x_www_form_urlencoded_tuple("redirect_to", redirect_to)
            .x_www_form_urlencoded_tuple("testcookie", "1");
        let then = then.status(status).body(body);
        if let Some(location) = location {
            then.header("location", location);
        }
    });
    (get, post)
}

// Serve `page` at `/{path}`.
fn page<'a>(server: &'a MockServer, path: &str, status: u16, body: &str) -> Mock<'a> {
    let body = body.to_string();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(status).body(body);
    })
}

// Answer a comment on `/{url}` carrying `fields`, and passing `check`, with
// `status` (a redirect to the post when 302) and `body`.
fn mock_comment<'a>(
    server: &'a MockServer,
    url: &str,
    fields: &[(&str, &str)],
    check: fn(&HttpMockRequest) -> bool,
    status: u16,
    body: &str,
) -> Mock<'a> {
    let body = body.to_string();
    server.mock(|when, then| {
        let mut when = when.method(POST).path(format!("/{}", url)).matches(check);
        for (key, value) in fields {
            when = when.x_www_form_urlencoded_tuple(*key, *value);
        }
        let then = then.status(status).body(body);
        if status == 302 {
            then.header("location", "/hello-world/#comment-9");
        }
    })
}

// The body of a request, as text.
fn body(request: &HttpMockRequest) -> String {
    String::from_utf8_lossy(request.body.as_deref().unwrap_or_default()).to_string()
}

// The value of `name` in a form encoded body.
fn form_field(body: &str, name: &str) -> Option<String> {
    body.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| value.replace('+', " "))
    })
}

// An anonymous comment of ten random words, without the logged in nonce.
fn random_anonymous_comment(request: &HttpMockRequest) -> bool {
    let body = body(request);
    form_field(&body, "comment").is_some_and(|c| c.split(' ').count() == 10)
        && !body.contains("_wp_unfiltered_html_comment")
}

// A logged in user's comment, without a name or email.
fn logged_in_comment(request: &HttpMockRequest) -> bool {
    let body = body(request);
    !body.contains("author=") && !body.contains("email=")
}

// A search of every post type.
fn no_post_type(request: &HttpMockRequest) -> bool {
    !request
        .query_params
        .as_ref()
        .is_some_and(|q| q.iter().any(|(k, _)| k == "post_type"))
}

// Any comment.
fn any_comment(_: &HttpMockRequest) -> bool {
    true
}

// Serve a sitemap index at `/{path}` listing the captured sitemaps.
fn mock_sitemap(server: &MockServer, path: &str) {
    let index = SITEMAP.to_string();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(index);
    });
}

// Answer a REST list of `route` at `page` with `links`, of `pages` pages.
fn mock_rest_list<'a>(
    server: &'a MockServer,
    route: &str,
    page: &str,
    per_page: &str,
    pages: &str,
    status: u16,
    links: &str,
) -> Mock<'a> {
    let links = links.to_string();
    let pages = pages.to_string();
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/{}", route))
            .query_param("page", page)
            .query_param("per_page", per_page)
            .query_param("_fields", "link");
        then.status(status)
            .header("content-type", "application/json")
            .header("X-WP-TotalPages", pages)
            .body(links);
    })
}

#[tokio::test]
async fn wordpress_flows() {
    let server = MockServer::start();
    let dashboard_redirect = "/wp-admin/";
    let landing = |prefix: &str| format!("/{}/wp-admin/", prefix);

    // Log in.
    let mut posts = Vec::new();
    let mut gets = Vec::new();
    for (behaviour, prefix, page_html, answer) in [
        (
            LOG_IN,
            "login",
            LOGIN_PAGE,
            (302, Some(landing("login")), ""),
        ),
        (LOG_IN_WRONG, "wrong", LOGIN_PAGE, (200, None, LOGIN_FAILED)),
        (
            LOG_IN_NO_COOKIE,
            "nocookie",
            LOGIN_PAGE,
            (200, None, LOGIN_NO_COOKIE),
        ),
        (
            LOG_IN_NOT_REDIRECTED,
            "notredirected",
            LOGIN_PAGE,
            (200, None, "<html><body>Welcome</body></html>"),
        ),
        (
            LOG_IN_NO_FORM,
            "noform",
            r#"<html><body><div id="loginform">Log in elsewhere</div></body></html>"#,
            (302, Some(landing("noform")), ""),
        ),
        (
            LOG_IN_POST_STATUS,
            "poststatus",
            LOGIN_PAGE,
            (503, None, "busy"),
        ),
        (
            LOG_IN_PAGE_VALIDATION,
            "pagevalidation",
            LOGIN_PAGE,
            (302, Some(landing("pagevalidation")), ""),
        ),
        (
            LOG_IN_LOGGED_IN_VALIDATION,
            "landingvalidation",
            LOGIN_PAGE,
            (302, Some(landing("landingvalidation")), ""),
        ),
    ] {
        let (status, location, body) = answer;
        let (get, post) = mock_log_in(
            &server,
            prefix,
            page_html,
            dashboard_redirect,
            (status, location.as_deref(), body),
        );
        page(&server, &format!("{}/wp-admin/", prefix), 200, DASHBOARD);
        gets.push((behaviour, get));
        posts.push((behaviour, post));
    }
    let custom_redirect = format!("{}/elsewhere/", server.base_url());
    let (custom_get, custom_post) = mock_log_in(
        &server,
        "custom",
        LOGIN_PAGE,
        &custom_redirect,
        (302, Some("/elsewhere/"), ""),
    );
    page(&server, "elsewhere/", 200, DASHBOARD);
    page(&server, "pagestatus/wp-login.php", 500, "down");

    // Log out: each behaviour's dashboard carries its own log out nonce.
    page(
        &server,
        "logout/wp-admin/",
        200,
        &dashboard("rest", "good00good0"),
    );
    page(
        &server,
        "stale/wp-admin/",
        200,
        &dashboard("rest", "stale0stale"),
    );
    page(
        &server,
        "nolink/wp-admin/",
        200,
        "<html><body>Dashboard</body></html>",
    );
    let log_out = server.mock(|when, then| {
        when.method(GET)
            .path("/wp-login.php")
            .query_param("action", "logout")
            .query_param("_wpnonce", "good00good0");
        then.status(302)
            .header("location", "/wp-login.php?loggedout=true");
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/wp-login.php")
            .query_param("action", "logout")
            .query_param("_wpnonce", "stale0stale");
        then.status(200).body(LOGOUT_CONFIRM);
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/wp-login.php")
            .query_param("loggedout", "true");
        then.status(200).body(LOGGED_OUT);
    });

    // Search.
    let search = server.mock(|when, then| {
        when.method(GET)
            .path("/search/")
            .query_param("s", "foo bar & baz")
            .query_param("post_type", "page");
        then.status(200).body(SEARCH_RESULTS);
    });
    let search_defaults = server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .query_param("s", "")
            .matches(no_post_type);
        then.status(200).body(SEARCH_RESULTS);
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/searchvalidation/")
            .query_param("s", "foo");
        then.status(200).body(SEARCH_RESULTS);
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/searchstatus/")
            .query_param("s", "foo");
        then.status(500).body("down");
    });

    // Comments.
    let logged_in_nonce = fixture_value(COMMENT_LOGGED_IN, "_wp_unfiltered_html_comment_disabled");
    for post in [
        "anonymous/",
        "custom/",
        "duplicate/",
        "flood/",
        "notaccepted/",
        "postvalidation/",
        "commentedvalidation/",
    ] {
        page(&server, post, 200, COMMENT_ANONYMOUS);
    }
    page(&server, "loggedin/", 200, COMMENT_LOGGED_IN);
    page(
        &server,
        "closed/",
        200,
        "<html><body><p>Comments are closed.</p></body></html>",
    );
    page(&server, "hello-world/", 200, COMMENT_ANONYMOUS);
    let anonymous_fields = [
        ("comment_post_ID", "1"),
        ("comment_parent", "0"),
        ("author", "Goose"),
        ("email", "goose@example.com"),
    ];
    let comment_anonymous = mock_comment(
        &server,
        "anonymous/wp-comments-post.php",
        &anonymous_fields,
        random_anonymous_comment,
        302,
        "",
    );
    let comment_logged_in = mock_comment(
        &server,
        "loggedin/wp-comments-post.php",
        &[
            ("comment_post_ID", "1"),
            ("_wp_unfiltered_html_comment", &logged_in_nonce),
        ],
        logged_in_comment,
        302,
        "",
    );
    let comment_custom = mock_comment(
        &server,
        "custom/comments",
        &[
            ("comment_post_ID", "1"),
            ("comment_parent", "7"),
            ("comment", "Nice post"),
            ("author", "Ada"),
            ("email", "ada@example.com"),
        ],
        any_comment,
        302,
        "",
    );
    for (url, status, body) in [
        ("duplicate/wp-comments-post.php", 409, COMMENT_DUPLICATE),
        ("flood/wp-comments-post.php", 429, COMMENT_FLOOD),
        (
            "notaccepted/wp-comments-post.php",
            200,
            "<html><body><p>Please type your comment text.</p></body></html>",
        ),
        ("postvalidation/wp-comments-post.php", 302, ""),
        ("commentedvalidation/wp-comments-post.php", 302, ""),
    ] {
        mock_comment(&server, url, &anonymous_fields, any_comment, status, body);
    }

    // REST.
    let create = server.mock(|when, then| {
        when.method(POST)
            .path("/create/wp-json/wp/v2/posts")
            .header("X-WP-Nonce", "rest0nonce")
            .header("content-type", "application/json")
            .json_body(serde_json::json!({"title": "Goose", "status": "draft"}));
        then.status(201)
            .header("content-type", "application/json")
            .body(REST_POST);
    });
    let update = server.mock(|when, then| {
        when.method(POST)
            .path("/update/wp-json/wp/v2/posts/1")
            .header("X-WP-Nonce", "rest0nonce")
            .json_body(serde_json::json!({"title": "Hello world!"}));
        then.status(200)
            .header("content-type", "application/json")
            .body(REST_POST);
    });
    server.mock(|when, then| {
        when.method(POST).path("/stale/wp-json/wp/v2/posts/1");
        then.status(403)
            .header("content-type", "application/json")
            .body(REST_INVALID_NONCE);
    });
    server.mock(|when, then| {
        when.method(POST).path("/noid/wp-json/wp/v2/posts");
        then.status(201)
            .body(r#"{"code":"created","message":"but where?"}"#);
    });
    server.mock(|when, then| {
        when.method(POST).path("/notjson/wp-json/wp/v2/posts");
        then.status(200).body("<html><body>Not JSON</body></html>");
    });

    // Discovery: the sitemap's sub sitemaps sit at the root, as the index lists them.
    mock_sitemap(&server, "sitemap/wp-sitemap.xml");
    mock_sitemap(&server, "cap/wp-sitemap.xml");
    let sub_sitemap = |name: &str, body: String| {
        server.mock(|when, then| {
            when.method(GET).path(format!("/wp-sitemap-{}.xml", name));
            then.status(200).body(body);
        })
    };
    let posts_sitemap = sub_sitemap("posts-post-1", SITEMAP_POSTS.to_string());
    let urlset = |urls: &[&str]| {
        let urls: String = urls
            .iter()
            .map(|u| format!("<url><loc>http://example.com{}</loc></url>", u))
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset>{}</urlset>"#,
            urls
        )
    };
    sub_sitemap("posts-page-1", urlset(&["/about/", "/contact/"]));
    sub_sitemap("taxonomies-category-1", urlset(&["/category/news/"]));
    sub_sitemap(
        "taxonomies-post_tag-1",
        urlset(&["/tag/rust/", "/tag/wp/", "/tag/x/"]),
    );
    sub_sitemap("taxonomies-post_format-1", urlset(&["/type/gallery/"]));
    sub_sitemap("users-1", urlset(&["/author/admin/"]));
    server.mock(|when, then| {
        when.method(GET).path("/broken/wp-sitemap.xml");
        then.status(200).body(
            r#"<sitemapindex><sitemap><loc>/broken/wp-sitemap-posts-post-1.xml</loc></sitemap></sitemapindex>"#,
        );
    });
    server.mock(|when, then| {
        when.method(GET).path("/broken/wp-sitemap-posts-post-1.xml");
        then.status(500).body("down");
    });
    for prefix in ["rest", "restcap", "reststatus"] {
        page(
            &server,
            &format!("{}/wp-sitemap.xml", prefix),
            404,
            "Not found",
        );
    }
    let second_page: String = format!(
        "[{}]",
        (0..5)
            .map(|i| format!(r#"{{"link":"http://example.com/more-{}/"}}"#, i))
            .collect::<Vec<_>>()
            .join(",")
    );
    let rest_page_1 = mock_rest_list(
        &server,
        "rest/wp-json/wp/v2/posts",
        "1",
        "100",
        "2",
        200,
        REST_POSTS,
    );
    let rest_page_2 = mock_rest_list(
        &server,
        "rest/wp-json/wp/v2/posts",
        "2",
        "100",
        "2",
        200,
        &second_page,
    );
    for (route, links) in [
        ("pages", r#"[{"link":"/about/"}]"#),
        ("categories", r#"[]"#),
        ("tags", r#"[{"link":"/tag/rust/"}]"#),
    ] {
        mock_rest_list(
            &server,
            &format!("rest/wp-json/wp/v2/{}", route),
            "1",
            "100",
            "1",
            200,
            links,
        );
    }
    let rest_cap = mock_rest_list(
        &server,
        "restcap/wp-json/wp/v2/posts",
        "1",
        "4",
        "3",
        200,
        REST_POSTS,
    );
    mock_rest_list(
        &server,
        "reststatus/wp-json/wp/v2/posts",
        "1",
        "100",
        "1",
        401,
        r#"{"code":"rest_forbidden","message":"Sorry, you are not allowed to do that."}"#,
    );

    let metrics = run(
        &server.base_url(),
        vec![
            transaction(LOG_IN, |user| {
                Box::pin(async move {
                    let login = Login::builder()
                        .username("admin")
                        .password("secret")
                        .url("login/wp-login.php")
                        .build();
                    let html = wordpress::log_in(user, &login).await?;
                    if !html.contains("Dashboard") {
                        return Err(wrong("not the dashboard".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(LOG_IN_PATH, |user| {
                Box::pin(async move {
                    let login = Login::builder()
                        .username("admin")
                        .password("secret")
                        .url("custom/wp-login.php")
                        .redirect_to("/elsewhere/")
                        .build();
                    wordpress::log_in(user, &login).await?;
                    Ok(())
                })
            }),
            transaction(LOG_IN_WRONG, |user| {
                Box::pin(async move { log_in_at(user, "wrong", None, None).await })
            }),
            transaction(LOG_IN_NO_COOKIE, |user| {
                Box::pin(async move { log_in_at(user, "nocookie", None, None).await })
            }),
            transaction(LOG_IN_NOT_REDIRECTED, |user| {
                Box::pin(async move { log_in_at(user, "notredirected", None, None).await })
            }),
            transaction(LOG_IN_NO_FORM, |user| {
                Box::pin(async move { log_in_at(user, "noform", None, None).await })
            }),
            transaction(LOG_IN_PAGE_STATUS, |user| {
                Box::pin(async move { log_in_at(user, "pagestatus", None, None).await })
            }),
            transaction(LOG_IN_POST_STATUS, |user| {
                Box::pin(async move { log_in_at(user, "poststatus", None, None).await })
            }),
            transaction(LOG_IN_PAGE_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Not the log in page").build();
                    log_in_at(user, "pagevalidation", Some(&validate), None).await
                })
            }),
            transaction(LOG_IN_LOGGED_IN_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Not the dashboard").build();
                    log_in_at(user, "landingvalidation", None, Some(&validate)).await
                })
            }),
            transaction(LOG_OUT, |user| {
                Box::pin(async move {
                    let html = wordpress::log_out(user, "logout/wp-admin/").await?;
                    if !html.contains("You are now logged out") {
                        return Err(wrong("not the logged out page".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(LOG_OUT_STALE, |user| {
                Box::pin(async move {
                    wordpress::log_out(user, "stale/wp-admin/").await?;
                    Ok(())
                })
            }),
            transaction(LOG_OUT_NO_LINK, |user| {
                Box::pin(async move {
                    wordpress::log_out(user, "nolink/wp-admin/").await?;
                    Ok(())
                })
            }),
            transaction(SEARCH, |user| {
                Box::pin(async move {
                    let params = SearchParams::builder()
                        .keys("foo bar & baz")
                        .url("search/")
                        .post_type("page")
                        .build();
                    let html = wordpress::search(user, &params).await?;
                    if !html.contains("search-results") {
                        return Err(wrong("not the results".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(SEARCH_DEFAULTS, |user| {
                Box::pin(async move {
                    wordpress::search(user, &SearchParams::builder().build()).await?;
                    Ok(())
                })
            }),
            transaction(SEARCH_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Not the results").build();
                    let params = SearchParams::builder()
                        .keys("foo")
                        .url("searchvalidation/")
                        .results_page_validation(&validate)
                        .build();
                    wordpress::search(user, &params).await?;
                    Ok(())
                })
            }),
            transaction(SEARCH_STATUS, |user| {
                Box::pin(async move {
                    let params = SearchParams::builder()
                        .keys("foo")
                        .url("searchstatus/")
                        .build();
                    wordpress::search(user, &params).await?;
                    Ok(())
                })
            }),
            transaction(COMMENT_ANONYMOUS_FIELDS, |user| {
                Box::pin(async move { comment_on(user, "anonymous/", None, None).await })
            }),
            transaction(COMMENT_LOGGED_IN_FIELDS, |user| {
                Box::pin(async move { comment_on(user, "loggedin/", None, None).await })
            }),
            transaction(COMMENT_CUSTOM, |user| {
                Box::pin(async move {
                    let comment = Comment::builder()
                        .post_url("custom/")
                        .url("custom/comments")
                        .text("Nice post")
                        .parent(7)
                        .author("Ada")
                        .email("ada@example.com")
                        .build();
                    wordpress::post_comment(user, &comment).await?;
                    Ok(())
                })
            }),
            transaction(COMMENT_DUPLICATE_409, |user| {
                Box::pin(async move { comment_on(user, "duplicate/", None, None).await })
            }),
            transaction(COMMENT_FLOOD_429, |user| {
                Box::pin(async move { comment_on(user, "flood/", None, None).await })
            }),
            transaction(COMMENT_CLOSED, |user| {
                Box::pin(async move { comment_on(user, "closed/", None, None).await })
            }),
            transaction(COMMENT_NOT_ACCEPTED, |user| {
                Box::pin(async move { comment_on(user, "notaccepted/", None, None).await })
            }),
            transaction(COMMENT_POST_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Another post").build();
                    comment_on(user, "postvalidation/", Some(&validate), None).await
                })
            }),
            transaction(COMMENT_COMMENTED_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Another post").build();
                    comment_on(user, "commentedvalidation/", None, Some(&validate)).await
                })
            }),
            transaction(CREATE_POST, |user| {
                Box::pin(async move {
                    let post = Post::builder()
                        .url("create/wp-json/wp/v2/posts")
                        .title("Goose")
                        .status("draft")
                        .build();
                    let created = wordpress::create_post(user, "rest0nonce", &post).await?;
                    if created["id"] != 1 {
                        return Err(wrong(format!("created {}", created["id"])));
                    }
                    Ok(())
                })
            }),
            transaction(UPDATE_POST, |user| {
                Box::pin(async move {
                    let post = Post::builder()
                        .url("update/wp-json/wp/v2/posts/")
                        .title("Hello world!")
                        .build();
                    wordpress::update_post(user, "rest0nonce", 1, &post).await?;
                    Ok(())
                })
            }),
            transaction(REST_STALE_NONCE, |user| {
                Box::pin(async move {
                    let post = Post::builder()
                        .url("stale/wp-json/wp/v2/posts")
                        .title("x")
                        .build();
                    wordpress::update_post(user, "0000000000", 1, &post).await?;
                    Ok(())
                })
            }),
            transaction(REST_NO_ID, |user| {
                Box::pin(async move {
                    let post = Post::builder().url("noid/wp-json/wp/v2/posts").build();
                    wordpress::create_post(user, "rest0nonce", &post).await?;
                    Ok(())
                })
            }),
            transaction(REST_NOT_JSON, |user| {
                Box::pin(async move {
                    let post = Post::builder().url("notjson/wp-json/wp/v2/posts").build();
                    wordpress::create_post(user, "rest0nonce", &post).await?;
                    Ok(())
                })
            }),
            transaction(DISCOVER_SITEMAP, |user| {
                Box::pin(async move {
                    let discover = Discover::builder()
                        .sitemap("sitemap/wp-sitemap.xml")
                        .build();
                    let content = wordpress::discover(user, &discover).await?;
                    let counts: Vec<(String, usize)> = content
                        .kinds()
                        .map(|k| (k.to_string(), content.urls(k).len()))
                        .collect();
                    let expected: Vec<(String, usize)> = [
                        ("category", 1),
                        ("page", 2),
                        ("post", 57),
                        ("post_format", 1),
                        ("post_tag", 3),
                        ("user", 1),
                    ]
                    .iter()
                    .map(|(k, n)| (k.to_string(), *n))
                    .collect();
                    if counts != expected || content.posts()[0] != "/hello-world/" {
                        return Err(wrong(format!("found {:?}", counts)));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_CAP, |user| {
                Box::pin(async move {
                    let discover = Discover::builder()
                        .sitemap("cap/wp-sitemap.xml")
                        .max_per_kind(2)
                        .build();
                    let content = wordpress::discover(user, &discover).await?;
                    let counts: Vec<usize> =
                        content.kinds().map(|k| content.urls(k).len()).collect();
                    if counts != [1, 2, 2, 1, 2, 1] {
                        return Err(wrong(format!("found {:?}", counts)));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_REST, |user| {
                Box::pin(async move {
                    let discover = Discover::builder()
                        .sitemap("rest/wp-sitemap.xml")
                        .rest("rest/wp-json/wp/v2/")
                        .build();
                    let content = wordpress::discover(user, &discover).await?;
                    if content.posts().len() != 15
                        || content.posts()[0] != "/hello-world/"
                        || content.posts()[14] != "/more-4/"
                        || content.pages() != ["/about/"]
                        || !content.categories().is_empty()
                        || content.tags() != ["/tag/rust/"]
                    {
                        return Err(wrong(format!("found {:?}", content)));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_REST_CAP, |user| {
                Box::pin(async move {
                    let discover = Discover::builder()
                        .sitemap("restcap/wp-sitemap.xml")
                        .rest("restcap/wp-json/wp/v2/")
                        .rest_kinds(&[("posts", "post")])
                        .max_per_kind(4)
                        .build();
                    let content = wordpress::discover(user, &discover).await?;
                    // The mock sends 10 links for 4 asked; the cap still holds.
                    if content.posts().len() != 4 {
                        return Err(wrong(format!("found {:?}", content)));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_SITEMAP_STATUS, |user| {
                Box::pin(async move {
                    let discover = Discover::builder().sitemap("broken/wp-sitemap.xml").build();
                    wordpress::discover(user, &discover).await?;
                    Ok(())
                })
            }),
            transaction(DISCOVER_REST_STATUS, |user| {
                Box::pin(async move {
                    let discover = Discover::builder()
                        .sitemap("reststatus/wp-sitemap.xml")
                        .rest("reststatus/wp-json/wp/v2/")
                        .build();
                    wordpress::discover(user, &discover).await?;
                    Ok(())
                })
            }),
        ],
    )
    .await;

    let login = |prefix: &str| format!("{}/wp-login.php", prefix);
    let mut problems: Vec<String> = vec![
        check_named(&metrics, LOG_IN, &[&login("login")], Expect::Pass),
        check_named(&metrics, LOG_IN_PATH, &[&login("custom")], Expect::Pass),
        check_named(
            &metrics,
            LOG_IN_WRONG,
            &[&login("wrong")],
            Expect::Fail("log in failed: Error: The password you entered for the username admin is incorrect."),
        ),
        check_named(
            &metrics,
            LOG_IN_NO_COOKIE,
            &[&login("nocookie")],
            Expect::Fail("log in failed: Error: Cookies are blocked"),
        ),
        check_named(
            &metrics,
            LOG_IN_NOT_REDIRECTED,
            &[&login("notredirected")],
            Expect::Fail("log in failed: not redirected"),
        ),
        check_named(
            &metrics,
            LOG_IN_NO_FORM,
            &[&login("noform")],
            Expect::Fail("no loginform on page"),
        ),
        check_named(&metrics, LOG_IN_PAGE_STATUS, &[&login("pagestatus")], Expect::Fail("500")),
        check_named(&metrics, LOG_IN_POST_STATUS, &[&login("poststatus")], Expect::Fail("503")),
        check_named(
            &metrics,
            LOG_IN_PAGE_VALIDATION,
            &[&login("pagevalidation")],
            Expect::Fail("title not found"),
        ),
        check_named(
            &metrics,
            LOG_IN_LOGGED_IN_VALIDATION,
            &[&login("landingvalidation")],
            Expect::Fail("title not found"),
        ),
        // The log out link's name is shared; its hits prove this behaviour's.
        check_named(&metrics, LOG_OUT, &["logout/wp-admin/"], Expect::Pass),
        check_named(
            &metrics,
            LOG_OUT_STALE,
            &["wp-login.php?action=logout"],
            Expect::Fail("log out not confirmed: You are attempting to log out of Theme Test Data"),
        ),
        check_named(
            &metrics,
            LOG_OUT_NO_LINK,
            &["nolink/wp-admin/"],
            Expect::Fail("no log out link on page"),
        ),
        // Every search is named `search`; the mocks' hits prove the passes.
        check_named(&metrics, SEARCH, &[], Expect::Pass),
        check_named(&metrics, SEARCH_DEFAULTS, &[], Expect::Pass),
        check_named(&metrics, SEARCH_VALIDATION, &["search"], Expect::Fail("title not found")),
        check_named(&metrics, SEARCH_STATUS, &["search"], Expect::Fail("500")),
        check_named(
            &metrics,
            COMMENT_ANONYMOUS_FIELDS,
            &["anonymous/wp-comments-post.php"],
            Expect::Pass,
        ),
        check_named(
            &metrics,
            COMMENT_LOGGED_IN_FIELDS,
            &["loggedin/wp-comments-post.php"],
            Expect::Pass,
        ),
        check_named(&metrics, COMMENT_CUSTOM, &["custom/comments"], Expect::Pass),
        check_named(
            &metrics,
            COMMENT_DUPLICATE_409,
            &["duplicate/wp-comments-post.php"],
            Expect::Fail("409"),
        ),
        check_named(
            &metrics,
            COMMENT_FLOOD_429,
            &["flood/wp-comments-post.php"],
            Expect::Fail("429"),
        ),
        check_named(
            &metrics,
            COMMENT_CLOSED,
            &["comment form page"],
            Expect::Fail("closed/: no comment form on page"),
        ),
        check_named(
            &metrics,
            COMMENT_NOT_ACCEPTED,
            &["notaccepted/wp-comments-post.php"],
            Expect::Fail("comment not accepted: Please type your comment text."),
        ),
        check_named(
            &metrics,
            COMMENT_POST_VALIDATION,
            &["comment form page"],
            Expect::Fail("title not found: Another post"),
        ),
        check_named(
            &metrics,
            COMMENT_COMMENTED_VALIDATION,
            &["commentedvalidation/wp-comments-post.php"],
            Expect::Fail("title not found: Another post"),
        ),
        check_named(&metrics, CREATE_POST, &["create/wp-json/wp/v2/posts"], Expect::Pass),
        check_named(&metrics, UPDATE_POST, &["update/wp-json/wp/v2/posts/{id}"], Expect::Pass),
        check_named(
            &metrics,
            REST_STALE_NONCE,
            &["stale/wp-json/wp/v2/posts/{id}"],
            Expect::Fail("403"),
        ),
        check_named(
            &metrics,
            REST_NO_ID,
            &["noid/wp-json/wp/v2/posts"],
            Expect::Fail("response has no post id"),
        ),
        check_named(
            &metrics,
            REST_NOT_JSON,
            &["notjson/wp-json/wp/v2/posts"],
            Expect::Fail("response is not JSON"),
        ),
        check_named(&metrics, DISCOVER_SITEMAP, &["wp-sitemap-posts-post-1.xml"], Expect::Pass),
        check_named(&metrics, DISCOVER_CAP, &["wp-sitemap-posts-post-1.xml"], Expect::Pass),
        check_named(
            &metrics,
            DISCOVER_REST,
            &["rest/wp-sitemap.xml", "rest/wp-json/wp/v2/posts"],
            Expect::Pass,
        ),
        check_named(&metrics, DISCOVER_REST_CAP, &["restcap/wp-json/wp/v2/posts"], Expect::Pass),
        check_named(
            &metrics,
            DISCOVER_SITEMAP_STATUS,
            &["/broken/wp-sitemap-posts-post-1.xml"],
            Expect::Fail("500"),
        ),
        check_named(
            &metrics,
            DISCOVER_REST_STATUS,
            &["reststatus/wp-json/wp/v2/posts"],
            Expect::Fail("401"),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Each mock that proves a request was made its expected number of times.
    let mut counts: Vec<(&str, &str, usize, usize)> = vec![
        (LOG_IN_PATH, "log in form loaded", custom_get.hits(), 1),
        (LOG_IN_PATH, "log in posted", custom_post.hits(), 1),
        (LOG_OUT, "log out link followed", log_out.hits(), 1),
        (SEARCH, "searched", search.hits(), 1),
        (SEARCH_DEFAULTS, "searched", search_defaults.hits(), 1),
        (
            COMMENT_ANONYMOUS_FIELDS,
            "commented",
            comment_anonymous.hits(),
            1,
        ),
        (
            COMMENT_LOGGED_IN_FIELDS,
            "commented",
            comment_logged_in.hits(),
            1,
        ),
        (COMMENT_CUSTOM, "commented", comment_custom.hits(), 1),
        (CREATE_POST, "created", create.hits(), 1),
        (UPDATE_POST, "updated", update.hits(), 1),
        (
            DISCOVER_SITEMAP,
            "posts sitemap read (twice with the cap)",
            posts_sitemap.hits(),
            2,
        ),
        (DISCOVER_REST, "first REST page read", rest_page_1.hits(), 1),
        (
            DISCOVER_REST,
            "second REST page read",
            rest_page_2.hits(),
            1,
        ),
        (
            DISCOVER_REST_CAP,
            "capped REST page read",
            rest_cap.hits(),
            1,
        ),
    ];
    for (behaviour, get) in &gets {
        let expected = if *behaviour == LOG_IN_PAGE_STATUS {
            0
        } else {
            1
        };
        counts.push((behaviour, "log in form loaded", get.hits(), expected));
    }
    for (behaviour, post) in &posts {
        let expected = match *behaviour {
            LOG_IN_NO_FORM | LOG_IN_PAGE_VALIDATION => 0,
            _ => 1,
        };
        counts.push((behaviour, "log in posted", post.hits(), expected));
    }
    for (behaviour, what, hits, expected) in counts {
        if hits != expected {
            problems.push(format!(
                "behaviour {}: {} {} times, expected {}",
                behaviour, what, hits, expected
            ));
        }
    }
    // A site without a sitemap records no error for it.
    if metrics
        .errors
        .values()
        .any(|e| e.name.ends_with("wp-sitemap.xml"))
    {
        problems.push(format!(
            "behaviour {}: the missing sitemap was recorded as an error",
            DISCOVER_REST
        ));
    }
    common::assert_all(problems);
}

// Log in as admin:secret at `/{prefix}/wp-login.php`.
async fn log_in_at(
    user: &mut GooseUser,
    prefix: &str,
    page_validation: Option<&Validate<'_>>,
    landing_validation: Option<&Validate<'_>>,
) -> TransactionResult {
    let url = format!("{}/wp-login.php", prefix);
    let mut login = Login::builder()
        .username("admin")
        .password("secret")
        .url(url.as_str());
    if let Some(validate) = page_validation {
        login = login.log_in_page_validation(validate);
    }
    if let Some(validate) = landing_validation {
        login = login.logged_in_page_validation(validate);
    }
    wordpress::log_in(user, &login.build()).await?;
    Ok(())
}

// Comment on `/{post}` through `/{post}wp-comments-post.php`.
async fn comment_on(
    user: &mut GooseUser,
    post: &str,
    post_validation: Option<&Validate<'_>>,
    commented_validation: Option<&Validate<'_>>,
) -> TransactionResult {
    let url = format!("{}wp-comments-post.php", post);
    let mut comment = Comment::builder().post_url(post).url(url.as_str());
    if let Some(validate) = post_validation {
        comment = comment.post_page_validation(validate);
    }
    if let Some(validate) = commented_validation {
        comment = comment.commented_page_validation(validate);
    }
    wordpress::post_comment(user, &comment.build()).await?;
    Ok(())
}
