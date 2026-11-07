// Shared by the `wordpress` doc examples, which pull it in with a hidden
// `# mod support { include!("../tests/doc/wordpress.rs"); }` line, so each example
// can run against a mock WordPress site serving pages captured from the theme
// test data site (examples/wordpress-sites/capture-fixtures.sh). It is not a test
// target of its own.
use goose::config::GooseConfiguration;
use goose::goose::get_base_url;
use goose::prelude::*;
use goose_eggs::wordpress::{Comment, Discover, Login, Post, SearchParams, SiteContent};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use std::future::Future;

const LOGIN_PAGE: &str = include_str!("../fixtures/wordpress/login.html");
const DASHBOARD: &str = include_str!("../fixtures/wordpress/dashboard.html");
const LOGGED_OUT: &str = include_str!("../fixtures/wordpress/logged-out.html");
const SEARCH_RESULTS: &str = include_str!("../fixtures/wordpress/search-results.html");
const COMMENT_ANONYMOUS: &str = include_str!("../fixtures/wordpress/comment-form-anonymous.html");
const COMMENT_LOGGED_IN: &str = include_str!("../fixtures/wordpress/comment-form-logged-in.html");
const SITEMAP: &str = include_str!("../fixtures/wordpress/sitemap.xml");
const SITEMAP_POSTS: &str = include_str!("../fixtures/wordpress/sitemap-posts-post-1.xml");
const EDITOR_NEW: &str = include_str!("../fixtures/wordpress/editor-new.html");
const EDITOR_EDIT: &str = include_str!("../fixtures/wordpress/editor-edit.html");
const BROWSER_POST: &str = include_str!("../fixtures/wordpress/browser-post.html");

/// A hidden field's value in a fixture, attributes in any order. Written apart
/// from the library's parsers, so a broken parser cannot agree with it.
pub fn fixture_value(html: &str, name: &str) -> String {
    let input = regex::Regex::new(r"(?s)<input\b[^>]*>").unwrap();
    let named = regex::Regex::new(&format!(r#"\bname=["']{}["']"#, regex::escape(name))).unwrap();
    let value = regex::Regex::new(r#"\bvalue=["']([^"']*)["']"#).unwrap();
    let tag = input
        .find_iter(html)
        .map(|m| m.as_str())
        .find(|tag| named.is_match(tag))
        .unwrap_or_else(|| panic!("fixture has no {} field", name));
    value.captures(tag).map(|c| c[1].to_string()).unwrap_or_default()
}

/// The log out nonce on the dashboard fixture.
pub fn log_out_nonce() -> String {
    regex::Regex::new(r"action=logout&#038;_wpnonce=([0-9a-f]+)")
        .unwrap()
        .captures(DASHBOARD)
        .unwrap()[1]
        .to_string()
}

/// A mock server and the mock whose hits an example counts.
pub struct Site {
    server: MockServer,
    counted: usize,
}

impl Site {
    fn hits(&self) -> usize {
        httpmock::Mock::new(self.counted, &self.server).hits()
    }
    /// Searches made with the expected parameters.
    pub fn searches(&self) -> usize {
        self.hits()
    }
    /// Comments posted with the expected fields.
    pub fn comments(&self) -> usize {
        self.hits()
    }
    /// Posts saved with the expected nonce and fields.
    pub fn saves(&self) -> usize {
        self.hits()
    }
}

impl std::ops::Deref for Site {
    type Target = MockServer;
    fn deref(&self) -> &MockServer {
        &self.server
    }
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

/// A site with WordPress's log in form at `/{path}`, which redirects a post of
/// `username`, `password` and the test cookie flag to `redirect_to` (the
/// form's own, the dashboard, when `redirect_to` is None).
// The preferences read the dashboard fixture's scripts make: its user has none saved.
fn mock_preferences(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/wp-json/wp/v2/users/me");
        then.status(200).header("content-type", "application/json").body(r#"{"id":1}"#);
    });
}

fn log_in_site_to(username: &str, password: &str, path: &str, redirect_to: Option<&str>) -> MockServer {
    let server = MockServer::start();
    mock_preferences(&server);
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200)
            .header("set-cookie", "wordpress_test_cookie=WP%20Cookie%20check; path=/")
            .body(LOGIN_PAGE);
    });
    let form_redirect = fixture_value(LOGIN_PAGE, "redirect_to");
    let redirect = match redirect_to {
        Some(path) => format!("{}{}", server.base_url(), path),
        None => form_redirect,
    };
    let landing = redirect_to.unwrap_or("/wp-admin/").to_string();
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}", path))
            .x_www_form_urlencoded_tuple("log", username)
            .x_www_form_urlencoded_tuple("pwd", password)
            .x_www_form_urlencoded_tuple("testcookie", "1")
            .x_www_form_urlencoded_tuple("redirect_to", &redirect);
        then.status(302).header("location", &landing);
    });
    server.mock(|when, then| {
        when.method(GET).path(&landing);
        then.status(200).body(DASHBOARD);
    });
    server
}

/// A site that logs in `username`:`password` at `/{path}`, landing on the dashboard.
pub fn log_in_site(username: &str, password: &str, path: &str) -> MockServer {
    log_in_site_to(username, password, path, None)
}

/// A site that logs in `username`:`password` at `/{path}`, landing on `redirect_to`.
pub fn log_in_site_redirect(username: &str, password: &str, path: &str, redirect_to: &str) -> MockServer {
    log_in_site_to(username, password, path, Some(redirect_to))
}

/// Log in with `login`; panic naming `example` on failure.
pub fn log_in(example: &str, server: &MockServer, login: &Login) {
    let mut user = user(server);
    let result = block_on(goose_eggs::wordpress::log_in(&mut user, login));
    if result.is_err() {
        panic!("doc example {}: log in failed: {}", example, failure(&result));
    }
}

/// Log in with `login`; panic naming `example` unless it fails with `text`.
pub fn log_in_fails(example: &str, server: &MockServer, login: &Login, text: &str) {
    let mut user = user(server);
    let result = block_on(goose_eggs::wordpress::log_in(&mut user, login));
    if !failure(&result).contains(text) {
        panic!("doc example {}: expected a failure with {:?}, got {}", example, text, failure(&result));
    }
}

/// A logged in site: the dashboard with its log out link, which logs out.
pub fn log_out_site() -> Site {
    let server = MockServer::start();
    mock_preferences(&server);
    server.mock(|when, then| {
        when.method(GET).path("/wp-admin/");
        then.status(200).body(DASHBOARD);
    });
    let counted = server
        .mock(|when, then| {
            when.method(GET)
                .path("/wp-login.php")
                .query_param("action", "logout")
                .query_param("_wpnonce", log_out_nonce());
            then.status(302).header("location", "/wp-login.php?loggedout=true");
        })
        .id;
    server.mock(|when, then| {
        when.method(GET).path("/wp-login.php").query_param("loggedout", "true");
        then.status(200).body(LOGGED_OUT);
    });
    Site { server, counted }
}

/// How many times the log out link was followed with its nonce.
pub fn log_out_hits(site: &Site) -> usize {
    site.hits()
}

/// A site answering a search for `keys` (and `post_type`) at `/{path}`.
pub fn search_site(path: &str, keys: &str, post_type: Option<&str>) -> Site {
    let server = MockServer::start();
    let counted = server
        .mock(|when, then| {
            let mut when = when.method(GET).path(format!("/{}", path)).query_param("s", keys);
            if let Some(post_type) = post_type {
                when = when.query_param("post_type", post_type);
            }
            then.status(200).body(SEARCH_RESULTS);
        })
        .id;
    Site { server, counted }
}

/// Search with `params`; panic naming `example` unless one expected search ran.
pub fn search(example: &str, site: &Site, params: &SearchParams) {
    let mut user = user(site);
    let result = block_on(goose_eggs::wordpress::search(&mut user, params));
    if result.is_err() || site.searches() != 1 {
        panic!(
            "doc example {}: search failed: {}, expected searches: {}",
            example,
            failure(&result),
            site.searches()
        );
    }
}

/// Search with `params`; panic naming `example` unless it fails with `text`.
pub fn search_fails(example: &str, site: &Site, params: &SearchParams, text: &str) {
    let mut user = user(site);
    let result = block_on(goose_eggs::wordpress::search(&mut user, params));
    if !failure(&result).contains(text) {
        panic!("doc example {}: expected a failure with {:?}, got {}", example, text, failure(&result));
    }
}

/// A site with a post at `/{post}` whose comment form posts to `/{url}`, and
/// answers a comment carrying `fields` (and the form's post ID) with a redirect
/// back to the post.
pub fn comment_site_at(post: &str, url: &str, logged_in: bool, fields: &[(&str, &str)]) -> Site {
    let server = MockServer::start();
    let page = if logged_in { COMMENT_LOGGED_IN } else { COMMENT_ANONYMOUS };
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", post));
        then.status(200).body(page);
    });
    let post_id = fixture_value(page, "comment_post_ID");
    let counted = server
        .mock(|when, then| {
            let mut when = when
                .method(POST)
                .path(format!("/{}", url))
                .x_www_form_urlencoded_tuple("comment_post_ID", &post_id)
                .x_www_form_urlencoded_key_exists("comment");
            for (key, value) in fields {
                when = when.x_www_form_urlencoded_tuple(*key, *value);
            }
            then.status(302).header("location", format!("/{}#comment-2", post));
        })
        .id;
    Site { server, counted }
}

/// A post at `/{post}` taking comments at `/wp-comments-post.php`.
pub fn comment_site(post: &str, logged_in: bool, fields: &[(&str, &str)]) -> Site {
    comment_site_at(post, "wp-comments-post.php", logged_in, fields)
}

/// Post `comment`; panic naming `example` unless one expected comment was posted.
pub fn post_comment(example: &str, site: &Site, comment: &Comment) {
    let mut user = user(site);
    let result = block_on(goose_eggs::wordpress::post_comment(&mut user, comment));
    if result.is_err() || site.comments() != 1 {
        panic!(
            "doc example {}: comment failed: {}, expected comments: {}",
            example,
            failure(&result),
            site.comments()
        );
    }
}

/// Post `comment`; panic naming `example` unless it fails with `text`.
pub fn post_comment_fails(example: &str, site: &Site, comment: &Comment, text: &str) {
    let mut user = user(site);
    let result = block_on(goose_eggs::wordpress::post_comment(&mut user, comment));
    if !failure(&result).contains(text) {
        panic!("doc example {}: expected a failure with {:?}, got {}", example, text, failure(&result));
    }
}

// The REST nonce an editor fixture carries.
fn editor_nonce(html: &str) -> String {
    regex::Regex::new(r#"createNonceMiddleware\( "([^"]+)""#)
        .unwrap()
        .captures(html)
        .unwrap()[1]
        .to_string()
}

/// A site with the block editor for `post_type` (`post` or `page`): its new
/// and edit pages, the reads their scripts make, and a save of a post titled
/// `title` with the editor's nonce, whose page then shows the title. Only the
/// save is counted.
pub fn editor_site(post_type: &str, title: &str) -> Site {
    editor_site_saving(post_type, title, &serde_json::json!({ "title": title }).to_string())
}

/// As `editor_site`, saving only a body that holds `fields`, as JSON.
pub fn editor_site_saving(post_type: &str, title: &str, fields: &str) -> Site {
    let server = MockServer::start();
    server.mock(|when, then| {
        let when = when.method(GET).path("/wp-admin/post-new.php");
        if post_type != "post" {
            when.query_param("post_type", post_type);
        }
        then.status(200).body(EDITOR_NEW);
    });
    server.mock(|when, then| {
        when.method(GET).path("/wp-admin/post.php").query_param("action", "edit");
        then.status(200).body(EDITOR_EDIT);
    });
    server.mock(|when, then| {
        when.method(GET).path("/wp-admin/admin-ajax.php");
        then.status(200).body("-1");
    });
    server.mock(|when, then| {
        when.method(GET).path_matches(regex::Regex::new(r"^/wp-json/wp/v2/").unwrap());
        then.status(200).header("content-type", "application/json").body("[]");
    });
    let route = if post_type == "post" { "posts" } else { "pages" };
    let answer = serde_json::json!({
        "id": 1841,
        "link": "/saved-post/",
        "title": { "raw": title, "rendered": title },
    })
    .to_string();
    let counted = server
        .mock(|when, then| {
            when.method(POST)
                .path_matches(regex::Regex::new(&format!(r"^/wp-json/wp/v2/{}/\d+$", route)).unwrap())
                .query_param("_locale", "user")
                .matches(|request| {
                    let nonce = request
                        .headers
                        .as_ref()
                        .and_then(|h| h.iter().find(|(k, _)| k.eq_ignore_ascii_case("X-WP-Nonce")))
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default();
                    nonce == editor_nonce(EDITOR_NEW) || nonce == editor_nonce(EDITOR_EDIT)
                })
                .json_body_partial(fields);
            then.status(200).header("content-type", "application/json").body(answer);
        })
        .id;
    let page = format!("<html><body><h1>{}</h1></body></html>", title);
    server.mock(|when, then| {
        when.method(GET).path("/saved-post/");
        then.status(200).body(page);
    });
    Site { server, counted }
}

/// Create `post` in the editor; panic naming `example` unless it was saved once.
pub fn create_post(example: &str, site: &Site, post: &Post) {
    let mut user = user(site);
    let result = block_on(goose_eggs::wordpress::create_post(&mut user, post));
    if result.is_err() || site.saves() != 1 {
        panic!(
            "doc example {}: create failed: {}, saves: {}",
            example,
            failure(&result),
            site.saves()
        );
    }
}

/// A small site to browse from `/{home}`: an About page in the navigation, two
/// posts and a category on the home page, and a tag on the first post.
pub fn crawl_site(home: &str) -> MockServer {
    let server = MockServer::start();
    let body = |classes: &str, inner: &str| {
        format!(r#"<html><body class="{}"><main>{}</main></body></html>"#, classes, inner)
    };
    let at = |path: &str| format!("/{}{}", home, path);
    let pages = [
        (
            at(""),
            format!(
                r#"<html><body class="home blog"><nav><a href="{}">About</a></nav><main><a href="{}">One</a><a href="{}">Two</a><a href="{}" rel="category tag">News</a></main></body></html>"#,
                at("about/"),
                at("one/"),
                at("two/"),
                at("category/news/")
            ),
        ),
        (at("about/"), body("page page-id-2", "")),
        (
            at("one/"),
            body("single single-post postid-1", &format!(r#"<a href="{}" rel="tag">rust</a>"#, at("tag/rust/"))),
        ),
        (at("two/"), body("single single-post postid-2", "")),
        (at("category/news/"), body("archive category", "")),
        (at("tag/rust/"), body("archive tag", "")),
    ];
    for (path, html) in pages {
        server.mock(|when, then| {
            when.method(GET).path(path);
            then.status(200).body(html);
        });
    }
    server
}

/// A post at `/{path}`, as WordPress serves it, with its assets counted.
pub fn page_site(path: &str) -> Site {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(BROWSER_POST);
    });
    let counted = server
        .mock(|when, then| {
            when.method(GET).path_matches(regex::Regex::new(r"^/wp-(includes|content)/").unwrap());
            then.status(200).body("");
        })
        .id;
    Site { server, counted }
}

/// How many of the site's assets were loaded.
pub fn assets_loaded(site: &Site) -> usize {
    site.hits()
}

/// A site with the core sitemap index at `/{path}`, the captured posts sitemap,
/// and one URL in each other sitemap the index lists.
pub fn sitemap_site(path: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(SITEMAP);
    });
    server.mock(|when, then| {
        when.method(GET).path("/wp-sitemap-posts-post-1.xml");
        then.status(200).body(SITEMAP_POSTS);
    });
    for (sitemap, url) in [
        ("posts-page-1", "/about/"),
        ("taxonomies-category-1", "/category/news/"),
        ("taxonomies-post_tag-1", "/tag/rust/"),
        ("taxonomies-post_format-1", "/type/gallery/"),
        ("users-1", "/author/admin/"),
    ] {
        server.mock(|when, then| {
            when.method(GET).path(format!("/wp-sitemap-{}.xml", sitemap));
            then.status(200).body(format!(
                r#"<?xml version="1.0" encoding="UTF-8"?><urlset><url><loc>{}</loc></url></urlset>"#,
                url
            ));
        });
    }
    server
}

/// Discover with `discover`; panic naming `example` on failure.
pub fn discover(example: &str, server: &MockServer, discover: &Discover) -> SiteContent {
    let mut user = user(server);
    let result = block_on(goose_eggs::wordpress::discover(&mut user, discover));
    match result {
        Ok(content) => content,
        Err(_) => panic!("doc example {}: discover failed: {}", example, failure(&result)),
    }
}
