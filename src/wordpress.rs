//! Functionality that's specific to WordPress.
//!
//! The helpers here drive any WordPress site through what WordPress itself
//! serves: the log in form at `wp-login.php`, the comment form, the search at
//! `?s=`, the core sitemap and the REST API. None of them assume a theme or any
//! content; every path is configurable on its builder.

use goose::goose::GooseResponse;
use goose::metrics::GooseRequestMetric;
use goose::prelude::*;
use log::warn;
use rand::seq::IndexedRandom;
use regex::Regex;
use std::collections::{BTreeMap, HashMap};

// Matches one opening tag of the named element, across lines.
fn tag_regex(tag: &str) -> Regex {
    Regex::new(&format!(r"(?is)<{}\b[^>]*>", regex::escape(tag))).unwrap()
}

// The decoded value of `name` in an opening tag, with the value in double,
// single or no quotes; `Some("")` for an attribute without a value.
pub(crate) fn attribute(tag: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"(?is)\s{}(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>/]+)))?(?:\s|/|>)"#,
        regex::escape(name)
    ))
    .unwrap();
    let capture = re.captures(tag)?;
    let value = capture
        .get(1)
        .or_else(|| capture.get(2))
        .or_else(|| capture.get(3))
        .map_or("", |m| m.as_str());
    Some(html_escape::decode_html_entities(value).to_string())
}

// Text with its tags (and the head, scripts and styles) removed and its white
// space collapsed, for error messages.
pub(crate) fn plain_text(html: &str) -> String {
    let hidden = Regex::new(r"(?is)<(head|script|style)\b.*?</(head|script|style)\s*>").unwrap();
    let tags = Regex::new(r"(?s)<[^>]*>").unwrap();
    let text = hidden.replace_all(html, " ");
    let text = tags.replace_all(&text, " ");
    html_escape::decode_html_entities(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Get the contents of the form whose `id` or `name` is `name`.
///
/// Attributes may come in any order and the form may span many lines. Returns an
/// empty [`String`] when the page has no such form.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::get_form;
///
/// let html = r#"
/// <form role="search" method="get" action="/"><input type="search" name="s"></form>
/// <form name="loginform" id="loginform" action="/wp-login.php" method="post">
///   <input type="text" name="log" id="user_login" value="" />
///   <input type="hidden" name="testcookie" value="1" />
/// </form>
/// "#;
///
/// let form = get_form(html, "loginform");
/// assert!(form.contains(r#"name="testcookie""#));
/// assert!(!form.contains(r#"name="s""#));
/// ```
pub fn get_form(html: &str, name: &str) -> String {
    find_form(html, name, |tag| {
        attribute(tag, "id").as_deref() == Some(name)
            || attribute(tag, "name").as_deref() == Some(name)
    })
}

// The contents of the form with `class` among its classes, as WooCommerce
// identifies some of its forms.
pub(crate) fn get_form_by_class(html: &str, class: &str) -> String {
    find_form(html, class, |tag| {
        attribute(tag, "class").is_some_and(|c| c.split_whitespace().any(|c| c == class))
    })
}

// The contents of the first form whose opening tag matches.
fn find_form(html: &str, name: &str, matches: impl Fn(&str) -> bool) -> String {
    let end = Regex::new(r"(?i)</form\s*>").unwrap();
    for form in tag_regex("form").find_iter(html) {
        if matches(form.as_str()) {
            let rest = &html[form.end()..];
            let contents = match end.find(rest) {
                Some(close) => &rest[..close.start()],
                None => rest,
            };
            return contents.to_string();
        }
    }
    warn!("form {} not found", name);
    "".to_string()
}

/// The successful controls of a form, as a browser submits them: every named
/// input, except checkboxes and radio buttons that are not checked and submit
/// buttons; every textarea; every select, with its selected option or its first.
pub(crate) fn form_controls(form: &str) -> Vec<(String, String)> {
    let mut controls = Vec::new();
    let control = Regex::new(
        r"(?is)<input\b[^>]*>|<textarea\b[^>]*>.*?</textarea\s*>|<select\b[^>]*>.*?</select\s*>",
    )
    .unwrap();
    let option = tag_regex("option");
    for m in control.find_iter(form) {
        let html = m.as_str();
        let open = &html[..html.find('>').map_or(html.len(), |i| i + 1)];
        let name = match attribute(open, "name") {
            Some(name) if !name.is_empty() => name,
            _ => continue,
        };
        let value = if html[1..].to_ascii_lowercase().starts_with("textarea") {
            let body = &html[open.len()..];
            let body = &body[..body.to_ascii_lowercase().rfind("</textarea").unwrap()];
            html_escape::decode_html_entities(body).to_string()
        } else if html[1..].to_ascii_lowercase().starts_with("select") {
            let options: Vec<&str> = option.find_iter(html).map(|o| o.as_str()).collect();
            let chosen = options
                .iter()
                .find(|o| attribute(o, "selected").is_some())
                .or_else(|| options.first());
            match chosen {
                Some(o) => attribute(o, "value").unwrap_or_default(),
                None => continue,
            }
        } else {
            let kind = attribute(open, "type")
                .unwrap_or_default()
                .to_ascii_lowercase();
            let checkable = kind == "checkbox" || kind == "radio";
            if (checkable && attribute(open, "checked").is_none())
                || kind == "submit"
                || kind == "button"
                || kind == "image"
            {
                continue;
            }
            attribute(open, "value").unwrap_or_else(|| {
                if checkable {
                    "on".to_string()
                } else {
                    "".to_string()
                }
            })
        };
        controls.push((name, value));
    }
    controls
}

/// Get the value of the form element named `name`: an input (a checkbox or radio
/// button only when checked), a textarea, or a select.
///
/// Attributes may come in any order, quoted with `"` or `'`, and the element may
/// span lines, as WordPress and its plugins write them. Returns `None` when there
/// is no such element.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::get_form_value;
///
/// // WordPress writes the comment form's hidden fields with single quotes, and
/// // puts the `id` before the `value` on some of them.
/// let form = r#"
/// <input type='hidden' name='comment_post_ID' value='1' id='comment_post_ID' />
/// <input type='hidden' name='comment_parent' id='comment_parent' value='0' />
/// "#;
///
/// assert_eq!(get_form_value(form, "comment_post_ID").as_deref(), Some("1"));
/// assert_eq!(get_form_value(form, "comment_parent").as_deref(), Some("0"));
/// assert_eq!(get_form_value(form, "missing"), None);
/// ```
pub fn get_form_value(form_html: &str, name: &str) -> Option<String> {
    let value = form_controls(form_html)
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v);
    if value.is_none() {
        warn!("form element {} not found", name);
    }
    value
}

/// Get the values of the named form elements, keyed by name. Elements the form
/// does not have are left out.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::{get_form, get_form_values};
///
/// let html = r#"<form id="commentform">
///   <input type='hidden' name='comment_post_ID' value='12' id='comment_post_ID' />
///   <input type='hidden' name='comment_parent' id='comment_parent' value='0' />
/// </form>"#;
///
/// let form = get_form(html, "commentform");
/// let values = get_form_values(&form, &["comment_post_ID", "comment_parent", "author"]);
/// assert_eq!(values.get("comment_post_ID").map(String::as_str), Some("12"));
/// assert_eq!(values.get("comment_parent").map(String::as_str), Some("0"));
/// assert!(!values.contains_key("author"));
/// ```
pub fn get_form_values<'a>(form: &str, elements: &'a [&str]) -> HashMap<&'a str, String> {
    let controls = form_controls(form);
    elements
        .iter()
        .filter_map(|&element| {
            controls
                .iter()
                .find(|(name, _)| name == element)
                .map(|(_, value)| (element, value.clone()))
        })
        .collect()
}

/// Get a nonce from the hidden field WordPress writes into its forms, by the
/// field's name: `_wpnonce` for most core forms, or the name a plugin chose (for
/// example `woocommerce-login-nonce`).
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::get_nonce;
///
/// let html = r#"<form method="post" action="options.php">
///   <input type="hidden" id="_wpnonce" name="_wpnonce" value="3f6e8a0b1c" />
///   <input type="hidden" name="_wp_http_referer" value="/wp-admin/options-general.php" />
/// </form>"#;
///
/// assert_eq!(get_nonce(html, "_wpnonce").as_deref(), Some("3f6e8a0b1c"));
/// assert_eq!(get_nonce(html, "other-nonce"), None);
/// ```
pub fn get_nonce(html: &str, field_name: &str) -> Option<String> {
    get_form_value(html, field_name).filter(|nonce| !nonce.is_empty())
}

/// Get the REST API nonce that WordPress gives a logged in user on admin pages,
/// which REST requests send back in an `X-WP-Nonce` header.
///
/// WordPress writes it into `wpApiSettings` and into the block editor's
/// `createNonceMiddleware("...")` call; either is read.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::get_rest_nonce;
///
/// let html = r#"<script id="wp-api-fetch-js-after">
/// wp.apiFetch.nonceMiddleware = wp.apiFetch.createNonceMiddleware( "e84e9447f9" );
/// </script>"#;
/// assert_eq!(get_rest_nonce(html).as_deref(), Some("e84e9447f9"));
///
/// let html = r#"var wpApiSettings = {"root":"/wp-json/","nonce":"d7e5418ef8","versionString":"wp/v2/"};"#;
/// assert_eq!(get_rest_nonce(html).as_deref(), Some("d7e5418ef8"));
/// ```
pub fn get_rest_nonce(html: &str) -> Option<String> {
    let patterns = [
        r#"createNonceMiddleware\(\s*["']([^"']+)["']\s*\)"#,
        r#"wpApiSettings\s*=\s*\{[^;]*?"nonce"\s*:\s*"([^"]+)""#,
    ];
    for pattern in patterns {
        if let Some(capture) = Regex::new(pattern).unwrap().captures(html) {
            return Some(capture[1].to_string());
        }
    }
    warn!("REST nonce not found");
    None
}

/// Get the log out link WordPress shows a logged in user, with its `_wpnonce`,
/// decoded so it can be requested as is.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::get_log_out_url;
///
/// let html = r#"<a class="ab-item" href="/wp-login.php?action=logout&#038;_wpnonce=6e935bd99e">Log Out</a>"#;
/// assert_eq!(
///     get_log_out_url(html).as_deref(),
///     Some("/wp-login.php?action=logout&_wpnonce=6e935bd99e")
/// );
/// ```
pub fn get_log_out_url(html: &str) -> Option<String> {
    let re =
        Regex::new(r#"(?i)href\s*=\s*["']([^"']*wp-login\.php\?action=logout[^"']*)["']"#).unwrap();
    match re.captures(html) {
        Some(capture) => Some(html_escape::decode_html_entities(&capture[1]).to_string()),
        None => {
            warn!("log out link not found");
            None
        }
    }
}

// Encode a query string value as a form does: unreserved characters as is,
// spaces as `+`, everything else as `%XX`.
pub(crate) fn query_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

// Fail the transaction when the request itself failed (an error status or no
// response); Goose has already recorded why.
pub(crate) fn request_ok(goose: &GooseResponse) -> Result<(), Box<TransactionError>> {
    if goose.request.success {
        Ok(())
    } else {
        Err(Box::new(TransactionError::RequestFailed {
            raw_request: Box::new(goose.request.clone()),
        }))
    }
}

// Validate a page and load its static assets, failing first if the request failed.
pub(crate) async fn load_page(
    user: &mut GooseUser,
    goose: GooseResponse,
    validate: Option<&crate::Validate<'_>>,
) -> Result<String, Box<TransactionError>> {
    request_ok(&goose)?;
    let none = crate::Validate::none();
    crate::validate_and_load_static_assets(user, goose, validate.unwrap_or(&none)).await
}

// GET `path`, recorded under `name`.
pub(crate) async fn get_named(
    user: &mut GooseUser,
    path: &str,
    name: &str,
) -> Result<GooseResponse, Box<TransactionError>> {
    let request = GooseRequest::builder().path(path).name(name).build();
    user.request(request).await
}

// The response body and headers, or an empty body when there was no response.
pub(crate) async fn read_response(goose: GooseResponse) -> (http::HeaderMap, String) {
    match goose.response {
        Ok(response) => {
            let headers = response.headers().clone();
            (headers, response.text().await.unwrap_or_default())
        }
        Err(e) => (http::HeaderMap::new(), e.to_string()),
    }
}

// A request with a JSON body and extra headers, recorded under `name`.
pub(crate) async fn send_json(
    user: &mut GooseUser,
    method: GooseMethod,
    path: &str,
    name: &str,
    headers: &[(&str, &str)],
    body: Option<&serde_json::Value>,
) -> Result<GooseResponse, Box<TransactionError>> {
    let mut builder = user.get_request_builder(&method, path)?;
    for (header, value) in headers {
        builder = builder.header(*header, *value);
    }
    if let Some(body) = body {
        builder = builder
            .header("Content-Type", "application/json")
            .body(body.to_string());
    }
    let request = GooseRequest::builder()
        .method(method)
        .path(path)
        .name(name)
        .set_request_builder(builder)
        .build();
    user.request(request).await
}

// Parse a JSON response, failing the request when it is not JSON or the
// request failed; a WordPress REST error's `code` and `message` name the failure.
pub(crate) async fn json_response(
    user: &mut GooseUser,
    goose: GooseResponse,
) -> Result<(http::HeaderMap, serde_json::Value), Box<TransactionError>> {
    let mut request = goose.request.clone();
    let (headers, body) = read_response(goose).await;
    let json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    if !request.success {
        let reason = match json.as_ref() {
            Some(json) => rest_error(json),
            None => plain_text(&body),
        };
        warn!("{}: {}", request.final_url, reason);
        // Goose recorded the status as the error; the log has the REST error.
        user.log_debug(&reason, Some(&request), Some(&headers), Some(&body))?;
        return Err(Box::new(TransactionError::RequestFailed {
            raw_request: Box::new(request),
        }));
    }
    match json {
        Some(json) => Ok((headers, json)),
        None => fail(
            user,
            &format!("{}: response is not JSON", request.final_url),
            &mut request,
            Some(&headers),
            Some(&body),
        ),
    }
}

// "code: message" from a WordPress REST error body.
pub(crate) fn rest_error(json: &serde_json::Value) -> String {
    let code = json.get("code").and_then(|c| c.as_str()).unwrap_or("error");
    let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");
    format!("{}: {}", code, plain_text(message))
}

// Mark the request failed with `tag` and fail the transaction.
pub(crate) fn fail<T>(
    user: &mut GooseUser,
    tag: &str,
    request: &mut GooseRequestMetric,
    headers: Option<&http::HeaderMap>,
    body: Option<&str>,
) -> Result<T, Box<TransactionError>> {
    user.set_failure(tag, request, headers, body)?;
    // set_failure always fails the transaction; this is never reached.
    Err(Box::new(TransactionError::RequestFailed {
        raw_request: Box::new(request.clone()),
    }))
}

// The value of a response header, if it is set and text.
pub(crate) fn header_value(headers: &http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Parameters that define how to log into a WordPress website and validate that
/// the user logged in. For complete documentation, refer to [`LoginBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Login;
///
/// let login: Login = Login::builder().username("admin").password("secret").build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::log_in_site("admin", "secret", "wp-login.php");
/// # support::log_in("Login", &server, &login);
/// ```
#[derive(Clone, Debug)]
pub struct Login<'a> {
    // The username or email address to log in with.
    username: &'a str,
    // The password to log in with.
    password: &'a str,
    // The path of the log in form.
    url: &'a str,
    // Where WordPress redirects after log in; empty uses the form's own value.
    redirect_to: &'a str,
    // Optional validation of the page with the log in form.
    log_in_page_validation: Option<&'a crate::Validate<'a>>,
    // Optional validation of the page WordPress redirects to after log in.
    logged_in_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> Login<'a> {
    /// Convenience function to bring [`LoginBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// let _login = Login::builder().username("editor").password("secret").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("editor", "secret", "wp-login.php");
    /// # support::log_in("Login::builder", &server, &_login);
    /// ```
    pub fn builder() -> LoginBuilder<'a> {
        LoginBuilder::new()
    }
}

/// Used to build a [`Login`] object, necessary to invoke the [`log_in`] function.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Login;
///
/// // Log in as `editor` with a custom password, on a site that moved its log in form.
/// let _login = Login::builder()
///     .username("editor")
///     .password("secret")
///     .url("custom/wp-login.php")
///     .build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::log_in_site("editor", "secret", "custom/wp-login.php");
/// # support::log_in("LoginBuilder", &server, &_login);
/// ```
pub struct LoginBuilder<'a> {
    username: &'a str,
    password: &'a str,
    url: &'a str,
    redirect_to: &'a str,
    log_in_page_validation: Option<&'a crate::Validate<'a>>,
    logged_in_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> LoginBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to a username of "username".
            username: "username",
            // Defaults to a password of "password".
            password: "password",
            // Defaults to WordPress's log in form.
            url: "wp-login.php",
            // Defaults to the form's own redirect_to, the dashboard.
            redirect_to: "",
            // Defaults to no extra validation.
            log_in_page_validation: None,
            // Defaults to no extra validation.
            logged_in_page_validation: None,
        }
    }

    /// The username, or email address, to log in with.
    ///
    /// Defaults to `username`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// // Log in as "editor" with the default password of "password".
    /// let _login = Login::builder().username("editor").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("editor", "password", "wp-login.php");
    /// # support::log_in("LoginBuilder::username", &server, &_login);
    /// ```
    pub fn username(mut self, username: impl Into<&'a str>) -> Self {
        self.username = username.into();
        self
    }

    /// The password to log in with.
    ///
    /// Defaults to `password`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// // Log in with the default username of "username" and the password "secret".
    /// let _login = Login::builder().password("secret").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("username", "secret", "wp-login.php");
    /// # support::log_in("LoginBuilder::password", &server, &_login);
    /// ```
    pub fn password(mut self, password: impl Into<&'a str>) -> Self {
        self.password = password.into();
        self
    }

    /// The path of the log in form.
    ///
    /// Defaults to `wp-login.php`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// // The site serves its log in form from a custom path.
    /// let _login = Login::builder().url("secret-door/wp-login.php").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("username", "password", "secret-door/wp-login.php");
    /// # support::log_in("LoginBuilder::url", &server, &_login);
    /// ```
    pub fn url(mut self, url: impl Into<&'a str>) -> Self {
        self.url = url.into();
        self
    }

    /// Where WordPress sends the user after logging in, a path on the site.
    ///
    /// Defaults to the `redirect_to` value in the log in form, which is the
    /// dashboard unless the form was linked with another.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// // Land on the home page instead of the dashboard.
    /// let _login = Login::builder().redirect_to("/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site_redirect("username", "password", "wp-login.php", "/");
    /// # support::log_in("LoginBuilder::redirect_to", &server, &_login);
    /// ```
    pub fn redirect_to(mut self, redirect_to: impl Into<&'a str>) -> Self {
        self.redirect_to = redirect_to.into();
        self
    }

    /// Extra validation of the page with the log in form.
    ///
    /// Defaults to `None`. [`log_in`] always checks that the page has the log in
    /// form, `id="loginform"`, and loads the page's static assets.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{wordpress::Login, Validate};
    ///
    /// let validate = Validate::builder().title("Log In").build();
    /// let _login = Login::builder().log_in_page_validation(&validate).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("username", "password", "wp-login.php");
    /// # support::log_in("LoginBuilder::log_in_page_validation", &server, &_login);
    /// # let strict = Validate::builder().title("Not the log in page").build();
    /// # let login = Login::builder().log_in_page_validation(&strict).build();
    /// # support::log_in_fails("LoginBuilder::log_in_page_validation", &server, &login, "title not found");
    /// ```
    pub fn log_in_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.log_in_page_validation = Some(validation);
        self
    }

    /// Extra validation of the page WordPress redirects to after log in.
    ///
    /// Defaults to `None`. [`log_in`] always checks that WordPress redirected and
    /// that the page shows no log in error, and loads the page's static assets.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{wordpress::Login, Validate};
    ///
    /// // The default redirect lands on the dashboard.
    /// let validate = Validate::builder().title("Dashboard").build();
    /// let _login = Login::builder().logged_in_page_validation(&validate).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("username", "password", "wp-login.php");
    /// # support::log_in("LoginBuilder::logged_in_page_validation", &server, &_login);
    /// # let strict = Validate::builder().title("Not the dashboard").build();
    /// # let login = Login::builder().logged_in_page_validation(&strict).build();
    /// # support::log_in_fails("LoginBuilder::logged_in_page_validation", &server, &login, "title not found");
    /// ```
    pub fn logged_in_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.logged_in_page_validation = Some(validation);
        self
    }

    /// Build the [`Login`] object which is then passed to the [`log_in`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Login;
    ///
    /// let _login = Login::builder().username("admin").password("secret").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::log_in_site("admin", "secret", "wp-login.php");
    /// # support::log_in("LoginBuilder::build", &server, &_login);
    /// ```
    pub fn build(self) -> Login<'a> {
        let Self {
            username,
            password,
            url,
            redirect_to,
            log_in_page_validation,
            logged_in_page_validation,
        } = self;
        Login {
            username,
            password,
            url,
            redirect_to,
            log_in_page_validation,
            logged_in_page_validation,
        }
    }
}

/// Log into a WordPress website.
///
/// Loads the log in form first, which sets the `wordpress_test_cookie` cookie
/// WordPress insists on, then posts the form with `testcookie=1`. WordPress
/// answers a failed log in with a 200 and an error box instead of an error
/// status, so the transaction fails when the post is not redirected or the page
/// shows `id="login_error"`; the failure names WordPress's message.
///
/// Returns the page WordPress redirected to.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::{log_in, Login};
///
/// transaction!(login).set_on_start();
///
/// async fn login(user: &mut GooseUser) -> TransactionResult {
///     let login = Login::builder()
///         .username("editor")
///         .password("secret")
///         .build();
///     let _dashboard = log_in(user, &login).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::log_in_site("editor", "secret", "wp-login.php");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(login(&mut user));
/// # assert!(result.is_ok(), "doc example log_in: {}", support::failure(&result));
/// ```
pub async fn log_in(
    user: &mut GooseUser,
    login: &Login<'_>,
) -> Result<String, Box<TransactionError>> {
    // Load the log in form; this also sets WordPress's test cookie.
    let default_validation = crate::Validate::builder().text(r#"id="loginform""#).build();
    let validate = login.log_in_page_validation.unwrap_or(&default_validation);
    let goose = user.get(login.url).await?;
    let mut login_request = goose.request.clone();
    let login_page = load_page(user, goose, Some(validate)).await?;

    let form = get_form(&login_page, "loginform");
    if form.is_empty() {
        return fail(
            user,
            &format!("{}: no loginform on page", login.url),
            &mut login_request,
            None,
            Some(&login_page),
        );
    }
    let redirect_to = if login.redirect_to.is_empty() {
        get_form_value(&form, "redirect_to").unwrap_or_default()
    } else {
        user.build_url(login.redirect_to)?
    };
    let params = [
        ("log", login.username.to_string()),
        ("pwd", login.password.to_string()),
        (
            "wp-submit",
            get_form_value(&form, "wp-submit").unwrap_or_else(|| "Log In".to_string()),
        ),
        ("redirect_to", redirect_to),
        ("testcookie", "1".to_string()),
    ];
    let goose = user.post_form(login.url, &params).await?;
    request_ok(&goose)?;

    // A failed log in is not redirected, and shows WordPress's error box.
    let mut request = goose.request.clone();
    let html = if goose.request.redirected {
        load_page(user, goose, login.logged_in_page_validation).await?
    } else {
        read_response(goose).await.1
    };
    if !request.redirected || html.contains(r#"id="login_error""#) {
        let error = Regex::new(r#"(?s)id="login_error"[^>]*>(.*?)</div>"#)
            .unwrap()
            .captures(&html)
            .map(|c| plain_text(&c[1]))
            .unwrap_or_else(|| "not redirected".to_string());
        return fail(
            user,
            &format!("{}: log in failed: {}", login.url, error),
            &mut request,
            None,
            Some(&html),
        );
    }
    Ok(html)
}

/// Log out of a WordPress website.
///
/// WordPress only logs a user out with the nonce in its log out link, so this
/// first loads `page`, a path showing the link to a logged in user (the admin bar
/// on most pages, or `wp-admin/`), then follows the link. Fails when the page has
/// no log out link, or WordPress did not confirm the log out (a stale nonce makes
/// it ask "Do you really want to log out?" instead).
///
/// Returns the page WordPress showed after logging out.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::log_out;
///
/// transaction!(logout).set_on_stop();
///
/// async fn logout(user: &mut GooseUser) -> TransactionResult {
///     let _logged_out = log_out(user, "wp-admin/").await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::log_out_site();
/// # let mut user = support::user(&server);
/// # let result = support::block_on(logout(&mut user));
/// # assert!(result.is_ok(), "doc example log_out: {}", support::failure(&result));
/// # assert_eq!(support::log_out_hits(&server), 1, "doc example log_out: log out link followed");
/// ```
pub async fn log_out(user: &mut GooseUser, page: &str) -> Result<String, Box<TransactionError>> {
    let goose = user.get(page).await?;
    let mut request = goose.request.clone();
    let html = load_page(user, goose, None).await?;
    let url = match get_log_out_url(&html) {
        Some(url) => url,
        None => {
            return fail(
                user,
                &format!("{}: no log out link on page", page),
                &mut request,
                None,
                Some(&html),
            )
        }
    };
    let goose = get_named(user, &url, "wp-login.php?action=logout").await?;
    request_ok(&goose)?;
    if goose.request.final_url.contains("loggedout=true") {
        return load_page(user, goose, None).await;
    }
    // A stale or missing nonce: WordPress asks to confirm instead.
    let mut request = goose.request.clone();
    let (headers, html) = read_response(goose).await;
    let message = plain_text(&html);
    let message = message
        .split(" Do you really want")
        .next()
        .unwrap_or_default();
    fail(
        user,
        &format!("{}: log out not confirmed: {}", request.final_url, message),
        &mut request,
        Some(&headers),
        Some(&html),
    )
}

/// Parameters that define how to make and validate a search. For complete
/// documentation, refer to [`SearchParamsBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::SearchParams;
///
/// let params: SearchParams = SearchParams::builder().keys("lorem").build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::search_site("", "lorem", None);
/// # support::search("SearchParams", &server, &params);
/// ```
#[derive(Clone, Debug)]
pub struct SearchParams<'a> {
    // The words to search for.
    keys: &'a str,
    // The path the search is made on.
    url: &'a str,
    // Optionally limit the search to one post type.
    post_type: Option<&'a str>,
    // Optionally validate the search results.
    results_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> SearchParams<'a> {
    /// Convenience function to bring [`SearchParamsBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SearchParams;
    ///
    /// let _params = SearchParams::builder().keys("hello").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("", "hello", None);
    /// # support::search("SearchParams::builder", &server, &_params);
    /// ```
    pub fn builder() -> SearchParamsBuilder<'a> {
        SearchParamsBuilder::new()
    }
}

/// Used to build a [`SearchParams`] object, necessary to invoke the [`search`] function.
///
/// # Example
/// ```rust
/// use goose_eggs::{wordpress::SearchParams, Validate};
///
/// // Search only pages, and check the results page says what it searched for.
/// let validate = Validate::builder().text("Search results").build();
/// let _params = SearchParams::builder()
///     .keys("about")
///     .post_type("page")
///     .results_page_validation(&validate)
///     .build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::search_site("", "about", Some("page"));
/// # support::search("SearchParamsBuilder", &server, &_params);
/// ```
pub struct SearchParamsBuilder<'a> {
    keys: &'a str,
    url: &'a str,
    post_type: Option<&'a str>,
    results_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> SearchParamsBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to empty search keys.
            keys: "",
            // Defaults to searching from the home page, as WordPress's search form does.
            url: "",
            // Defaults to every post type.
            post_type: None,
            // Defaults to no extra validation.
            results_page_validation: None,
        }
    }

    /// The words to search for, sent as `s`.
    ///
    /// Defaults to `""`, an empty search.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SearchParams;
    ///
    /// let _params = SearchParams::builder().keys("foo bar").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("", "foo bar", None);
    /// # support::search("SearchParamsBuilder::keys", &server, &_params);
    /// ```
    pub fn keys(mut self, keys: impl Into<&'a str>) -> Self {
        self.keys = keys.into();
        self
    }

    /// The path to search on.
    ///
    /// Defaults to `""`, the site's home page, where WordPress's own search form
    /// sends its searches.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SearchParams;
    ///
    /// // A site installed under /blog/.
    /// let _params = SearchParams::builder().keys("foo").url("blog/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("blog/", "foo", None);
    /// # support::search("SearchParamsBuilder::url", &server, &_params);
    /// ```
    pub fn url(mut self, url: impl Into<&'a str>) -> Self {
        self.url = url.into();
        self
    }

    /// Limit the search to one post type, sent as `post_type`.
    ///
    /// Defaults to `None`, every searchable post type.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SearchParams;
    ///
    /// let _params = SearchParams::builder().keys("foo").post_type("page").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("", "foo", Some("page"));
    /// # support::search("SearchParamsBuilder::post_type", &server, &_params);
    /// ```
    pub fn post_type(mut self, post_type: impl Into<&'a str>) -> Self {
        self.post_type = Some(post_type.into());
        self
    }

    /// Extra validation of the search results page.
    ///
    /// Defaults to `None`. [`search`] always fails on an error status, and loads
    /// the page's static assets.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{wordpress::SearchParams, Validate};
    ///
    /// let validate = Validate::builder().title("Search").build();
    /// let _params = SearchParams::builder()
    ///     .keys("foo")
    ///     .results_page_validation(&validate)
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("", "foo", None);
    /// # support::search("SearchParamsBuilder::results_page_validation", &server, &_params);
    /// # let strict = Validate::builder().title("Not the results").build();
    /// # let params = SearchParams::builder().keys("foo").results_page_validation(&strict).build();
    /// # support::search_fails("SearchParamsBuilder::results_page_validation", &server, &params, "title not found");
    /// ```
    pub fn results_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.results_page_validation = Some(validation);
        self
    }

    /// Build the [`SearchParams`] object which is then passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SearchParams;
    ///
    /// let _params = SearchParams::builder().keys("example keys").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::search_site("", "example keys", None);
    /// # support::search("SearchParamsBuilder::build", &server, &_params);
    /// ```
    pub fn build(self) -> SearchParams<'a> {
        let Self {
            keys,
            url,
            post_type,
            results_page_validation,
        } = self;
        SearchParams {
            keys,
            url,
            post_type,
            results_page_validation,
        }
    }
}

/// Search a WordPress website, the way its search form does: a GET of
/// `?s=<keys>`, with `post_type` when set.
///
/// The request is named `search`, whatever the keys, so every search shares one
/// line in the metrics. Returns the results page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress;
///
/// transaction!(search);
///
/// async fn search(user: &mut GooseUser) -> TransactionResult {
///     let params = wordpress::SearchParams::builder().keys("hello").build();
///     let _results = wordpress::search(user, &params).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::search_site("", "hello", None);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(search(&mut user));
/// # assert!(result.is_ok(), "doc example search: {}", support::failure(&result));
/// # assert_eq!(server.searches(), 1, "doc example search: searches made");
/// ```
pub async fn search(
    user: &mut GooseUser,
    params: &SearchParams<'_>,
) -> Result<String, Box<TransactionError>> {
    let mut path = format!("{}?s={}", params.url, query_encode(params.keys));
    if let Some(post_type) = params.post_type {
        path.push_str(&format!("&post_type={}", query_encode(post_type)));
    }
    let goose = get_named(user, &path, "search").await?;
    load_page(user, goose, params.results_page_validation).await
}

/// Parameters that define how to post a comment. For complete documentation,
/// refer to [`CommentBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Comment;
///
/// // A logged in user's comment: the form has no name or email to fill in.
/// let comment: Comment = Comment::builder().post_url("hello-world/").build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::comment_site("hello-world/", true, &[("_wp_unfiltered_html_comment", &support::fixture_value(include_str!("../tests/fixtures/wordpress/comment-form-logged-in.html"), "_wp_unfiltered_html_comment_disabled"))]);
/// # support::post_comment("Comment", &server, &comment);
/// ```
#[derive(Clone, Debug)]
pub struct Comment<'a> {
    // The path of the post with the comment form.
    post_url: &'a str,
    // The comment text; None writes random words.
    text: Option<&'a str>,
    // The name an anonymous commenter gives.
    author: &'a str,
    // The email address an anonymous commenter gives.
    email: &'a str,
    // The path the form posts to.
    url: &'a str,
    // The comment to reply to; None uses the form's value.
    parent: Option<u64>,
    // Optional validation of the page with the comment form.
    post_page_validation: Option<&'a crate::Validate<'a>>,
    // Optional validation of the page WordPress redirects to.
    commented_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> Comment<'a> {
    /// Convenience function to bring [`CommentBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder().post_url("hello-world/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[]);
    /// # support::post_comment("Comment::builder", &server, &_comment);
    /// ```
    pub fn builder() -> CommentBuilder<'a> {
        CommentBuilder::new()
    }
}

/// Used to build a [`Comment`] object, necessary to invoke the [`post_comment`] function.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Comment;
///
/// // An anonymous reply to comment 1, with fixed text.
/// let _comment = Comment::builder()
///     .post_url("hello-world/")
///     .text("Thanks for writing this up.")
///     .author("Ada")
///     .email("ada@example.com")
///     .parent(1)
///     .build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::comment_site("hello-world/", false, &[("comment", "Thanks for writing this up."), ("author", "Ada"), ("email", "ada@example.com"), ("comment_parent", "1")]);
/// # support::post_comment("CommentBuilder", &server, &_comment);
/// ```
pub struct CommentBuilder<'a> {
    post_url: &'a str,
    text: Option<&'a str>,
    author: &'a str,
    email: &'a str,
    url: &'a str,
    parent: Option<u64>,
    post_page_validation: Option<&'a crate::Validate<'a>>,
    commented_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> CommentBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to the home page; a post is almost always wanted.
            post_url: "",
            // Defaults to ten random words.
            text: None,
            // Defaults to a recognizable commenter.
            author: "Goose",
            email: "goose@example.com",
            // Defaults to WordPress's comment handler.
            url: "wp-comments-post.php",
            // Defaults to the form's own comment_parent, a top level comment.
            parent: None,
            // Defaults to no extra validation.
            post_page_validation: None,
            commented_page_validation: None,
        }
    }

    /// The path of the post (or page) whose comment form is used.
    ///
    /// Defaults to `""`, the home page; set it to a post, for example one picked
    /// with [`SiteContent::random`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder().post_url("2026/10/a-post/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("2026/10/a-post/", false, &[]);
    /// # support::post_comment("CommentBuilder::post_url", &server, &_comment);
    /// ```
    pub fn post_url(mut self, post_url: impl Into<&'a str>) -> Self {
        self.post_url = post_url.into();
        self
    }

    /// The comment text.
    ///
    /// Defaults to ten random words ([`crate::text::random_words`]), so repeated
    /// comments pass WordPress's duplicate comment check. Fixed text is refused
    /// as a duplicate the second time it is posted on a post.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder()
    ///     .post_url("hello-world/")
    ///     .text("A fixed comment.")
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[("comment", "A fixed comment.")]);
    /// # support::post_comment("CommentBuilder::text", &server, &_comment);
    /// ```
    pub fn text(mut self, text: impl Into<&'a str>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// The name an anonymous commenter gives; not sent when the form has no
    /// `author` field, as for a logged in user.
    ///
    /// Defaults to `Goose`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder().post_url("hello-world/").author("Ada").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[("author", "Ada")]);
    /// # support::post_comment("CommentBuilder::author", &server, &_comment);
    /// ```
    pub fn author(mut self, author: impl Into<&'a str>) -> Self {
        self.author = author.into();
        self
    }

    /// The email address an anonymous commenter gives; not sent when the form
    /// has no `email` field.
    ///
    /// Defaults to `goose@example.com`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder()
    ///     .post_url("hello-world/")
    ///     .email("ada@example.com")
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[("email", "ada@example.com")]);
    /// # support::post_comment("CommentBuilder::email", &server, &_comment);
    /// ```
    pub fn email(mut self, email: impl Into<&'a str>) -> Self {
        self.email = email.into();
        self
    }

    /// The path the comment form posts to.
    ///
    /// Defaults to `wp-comments-post.php`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// // A site installed under /blog/.
    /// let _comment = Comment::builder()
    ///     .post_url("blog/hello-world/")
    ///     .url("blog/wp-comments-post.php")
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site_at("blog/hello-world/", "blog/wp-comments-post.php", false, &[]);
    /// # support::post_comment("CommentBuilder::url", &server, &_comment);
    /// ```
    pub fn url(mut self, url: impl Into<&'a str>) -> Self {
        self.url = url.into();
        self
    }

    /// The ID of the comment to reply to, sent as `comment_parent`.
    ///
    /// Defaults to the form's own value, `0`, a top level comment.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder().post_url("hello-world/").parent(1).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[("comment_parent", "1")]);
    /// # support::post_comment("CommentBuilder::parent", &server, &_comment);
    /// ```
    pub fn parent(mut self, parent: u64) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Extra validation of the post page with the comment form.
    ///
    /// Defaults to `None`. [`post_comment`] always checks the page has the
    /// comment form, `id="commentform"`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{wordpress::Comment, Validate};
    ///
    /// let validate = Validate::builder().title("Hello world!").build();
    /// let _comment = Comment::builder()
    ///     .post_url("hello-world/")
    ///     .post_page_validation(&validate)
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[]);
    /// # support::post_comment("CommentBuilder::post_page_validation", &server, &_comment);
    /// # let strict = Validate::builder().title("Another post").build();
    /// # let comment = Comment::builder().post_url("hello-world/").post_page_validation(&strict).build();
    /// # support::post_comment_fails("CommentBuilder::post_page_validation", &server, &comment, "title not found");
    /// ```
    pub fn post_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.post_page_validation = Some(validation);
        self
    }

    /// Extra validation of the page WordPress redirects to once the comment is saved.
    ///
    /// Defaults to `None`. [`post_comment`] always checks that WordPress
    /// accepted the comment and redirected.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{wordpress::Comment, Validate};
    ///
    /// let validate = Validate::builder().title("Hello world!").build();
    /// let _comment = Comment::builder()
    ///     .post_url("hello-world/")
    ///     .commented_page_validation(&validate)
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[]);
    /// # support::post_comment("CommentBuilder::commented_page_validation", &server, &_comment);
    /// # let strict = Validate::builder().title("Another post").build();
    /// # let comment = Comment::builder().post_url("hello-world/").commented_page_validation(&strict).build();
    /// # support::post_comment_fails("CommentBuilder::commented_page_validation", &server, &comment, "title not found");
    /// ```
    pub fn commented_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.commented_page_validation = Some(validation);
        self
    }

    /// Build the [`Comment`] object which is then passed to the [`post_comment`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Comment;
    ///
    /// let _comment = Comment::builder().post_url("hello-world/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::comment_site("hello-world/", false, &[("author", "Goose")]);
    /// # support::post_comment("CommentBuilder::build", &server, &_comment);
    /// ```
    pub fn build(self) -> Comment<'a> {
        let Self {
            post_url,
            text,
            author,
            email,
            url,
            parent,
            post_page_validation,
            commented_page_validation,
        } = self;
        Comment {
            post_url,
            text,
            author,
            email,
            url,
            parent,
            post_page_validation,
            commented_page_validation,
        }
    }
}

/// Post a comment with a post's comment form.
///
/// Loads the post, reads `comment_post_ID` and `comment_parent` from its
/// `#commentform`, and posts the form to `wp-comments-post.php` with the comment
/// text, plus `author` and `email` when the form asks for them (anonymous users),
/// and `_wp_unfiltered_html_comment` when the form carries it (users who may post
/// unfiltered HTML; WordPress writes it as `_wp_unfiltered_html_comment_disabled`
/// and renames it with JavaScript, so it is sent under its real name).
///
/// WordPress refuses some comments a load test will make, each with an error
/// status that fails the transaction:
/// - **409**, a duplicate: the same text on the same post again. The default
///   random text avoids it.
/// - **429**, comment flood protection: a second comment from one IP address
///   within 15 seconds. Every Goose user shares the load test's address, so
///   anonymous comments must be paced (or posted by users that can moderate
///   comments, whom WordPress does not throttle).
/// - **403**, comments closed on the post; no form is shown either.
///
/// The request to the post is named `comment form page`, so comments on every
/// post share one line in the metrics. Returns the page WordPress redirected to.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::{post_comment, Comment};
///
/// transaction!(comment);
///
/// async fn comment(user: &mut GooseUser) -> TransactionResult {
///     let comment = Comment::builder().post_url("hello-world/").build();
///     let _post = post_comment(user, &comment).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::comment_site("hello-world/", false, &[("comment_post_ID", "1")]);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(comment(&mut user));
/// # assert!(result.is_ok(), "doc example post_comment: {}", support::failure(&result));
/// # assert_eq!(server.comments(), 1, "doc example post_comment: comments posted");
/// ```
pub async fn post_comment(
    user: &mut GooseUser,
    comment: &Comment<'_>,
) -> Result<String, Box<TransactionError>> {
    let goose = get_named(user, comment.post_url, "comment form page").await?;
    let mut request = goose.request.clone();
    let page = load_page(user, goose, comment.post_page_validation).await?;
    let form = get_form(&page, "commentform");
    let post_id = get_form_value(&form, "comment_post_ID");
    let post_id = match post_id {
        Some(id) if !form.is_empty() => id,
        _ => {
            return fail(
                user,
                &format!(
                    "{}: no comment form on page (comments closed?)",
                    comment.post_url
                ),
                &mut request,
                None,
                Some(&page),
            )
        }
    };
    let parent = match comment.parent {
        Some(parent) => parent.to_string(),
        None => get_form_value(&form, "comment_parent").unwrap_or_else(|| "0".to_string()),
    };
    let text = match comment.text {
        Some(text) => text.to_string(),
        None => crate::text::random_words(10),
    };
    let mut params = vec![
        ("comment_post_ID", post_id),
        ("comment_parent", parent),
        ("comment", text),
    ];
    let controls = form_controls(&form);
    let has = |name: &str| controls.iter().any(|(n, _)| n == name);
    if has("author") {
        params.push(("author", comment.author.to_string()));
    }
    if has("email") {
        params.push(("email", comment.email.to_string()));
    }
    if let Some(nonce) = get_form_value(&form, "_wp_unfiltered_html_comment")
        .or_else(|| get_form_value(&form, "_wp_unfiltered_html_comment_disabled"))
    {
        params.push(("_wp_unfiltered_html_comment", nonce));
    }
    let goose = user.post_form(comment.url, &params).await?;
    if goose.request.success && goose.request.redirected {
        return load_page(user, goose, comment.commented_page_validation).await;
    }
    // Refused: a duplicate (409), a flood (429), comments closed (403), or a
    // 200 error page for an empty comment. WordPress's page says which.
    let mut request = goose.request.clone();
    let (headers, html) = read_response(goose).await;
    let reason = Regex::new(r"(?s)<body[^>]*>(.*?)</body>")
        .unwrap()
        .captures(&html)
        .map(|c| plain_text(&c[1]))
        .unwrap_or_default();
    if !request.success {
        // Goose recorded the status as the error; the log has WordPress's reason.
        user.log_debug(&reason, Some(&request), Some(&headers), Some(&html))?;
        return Err(Box::new(TransactionError::RequestFailed {
            raw_request: Box::new(request),
        }));
    }
    fail(
        user,
        &format!("{}: comment not accepted: {}", comment.url, reason),
        &mut request,
        Some(&headers),
        Some(&html),
    )
}

/// The fields of a post to create or update through the REST API. For complete
/// documentation, refer to [`PostBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Post;
///
/// let post: Post = Post::builder().title("Draft").status("draft").build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"Draft","status":"draft"}"#);
/// # support::create_post("Post", &server, &post);
/// ```
#[derive(Clone, Debug)]
pub struct Post<'a> {
    // The REST route of the post type.
    url: &'a str,
    // The fields to send, as the REST API names them.
    fields: serde_json::Map<String, serde_json::Value>,
}
impl<'a> Post<'a> {
    /// Convenience function to bring [`PostBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder().title("A new title").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"A new title"}"#);
    /// # support::create_post("Post::builder", &server, &_post);
    /// ```
    pub fn builder() -> PostBuilder<'a> {
        PostBuilder::new()
    }
}

/// Used to build a [`Post`] object, necessary to invoke the [`create_post`] and
/// [`update_post`] functions.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Post;
///
/// // A published post with a title and content.
/// let _post = Post::builder()
///     .title("Load testing")
///     .content("<p>Written by Goose.</p>")
///     .status("publish")
///     .build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"Load testing","content":"<p>Written by Goose.</p>","status":"publish"}"#);
/// # support::create_post("PostBuilder", &server, &_post);
/// ```
pub struct PostBuilder<'a> {
    url: &'a str,
    fields: serde_json::Map<String, serde_json::Value>,
}
impl<'a> PostBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to the core posts route.
            url: "wp-json/wp/v2/posts",
            // Defaults to sending no fields.
            fields: serde_json::Map::new(),
        }
    }

    /// The post's title.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder().title("Hello").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"Hello"}"#);
    /// # support::create_post("PostBuilder::title", &server, &_post);
    /// ```
    pub fn title(self, title: &str) -> Self {
        self.field("title", serde_json::Value::from(title))
    }

    /// The post's content, as block markup or HTML.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder()
    ///     .content("<!-- wp:paragraph --><p>Hi</p><!-- /wp:paragraph -->")
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"content":"<!-- wp:paragraph --><p>Hi</p><!-- /wp:paragraph -->"}"#);
    /// # support::create_post("PostBuilder::content", &server, &_post);
    /// ```
    pub fn content(self, content: &str) -> Self {
        self.field("content", serde_json::Value::from(content))
    }

    /// The post's status: `draft`, `publish`, `pending`, `private` or `future`.
    ///
    /// WordPress makes a new post a `draft` when no status is sent.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder().status("publish").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"status":"publish"}"#);
    /// # support::create_post("PostBuilder::status", &server, &_post);
    /// ```
    pub fn status(self, status: &str) -> Self {
        self.field("status", serde_json::Value::from(status))
    }

    /// Any other field the REST API accepts, such as `excerpt`, `categories` or `meta`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder()
    ///     .field("categories", serde_json::json!([1, 3]))
    ///     .build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"categories":[1,3]}"#);
    /// # support::create_post("PostBuilder::field", &server, &_post);
    /// ```
    pub fn field(mut self, name: &str, value: serde_json::Value) -> Self {
        self.fields.insert(name.to_string(), value);
        self
    }

    /// The REST route of the post type.
    ///
    /// Defaults to `wp-json/wp/v2/posts`; pages are at `wp-json/wp/v2/pages`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _page = Post::builder().url("wp-json/wp/v2/pages").title("About").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/pages", r#"{"title":"About"}"#);
    /// # support::create_post("PostBuilder::url", &server, &_page);
    /// ```
    pub fn url(mut self, url: &'a str) -> Self {
        self.url = url;
        self
    }

    /// Build the [`Post`] object which is then passed to [`create_post`] or [`update_post`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Post;
    ///
    /// let _post = Post::builder().title("Built").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"Built"}"#);
    /// # support::create_post("PostBuilder::build", &server, &_post);
    /// ```
    pub fn build(self) -> Post<'a> {
        let Self { url, fields } = self;
        Post { url, fields }
    }
}

// Send a post's fields with the REST nonce, and check WordPress answered with a post.
async fn save_post(
    user: &mut GooseUser,
    nonce: &str,
    path: &str,
    name: &str,
    post: &Post<'_>,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let body = serde_json::Value::Object(post.fields.clone());
    let goose = send_json(
        user,
        GooseMethod::Post,
        path,
        name,
        &[("X-WP-Nonce", nonce)],
        Some(&body),
    )
    .await?;
    let mut request = goose.request.clone();
    let (headers, json) = json_response(user, goose).await?;
    if json.get("id").and_then(|id| id.as_u64()).is_none() {
        return fail(
            user,
            &format!("{}: response has no post id", path),
            &mut request,
            Some(&headers),
            Some(&json.to_string()),
        );
    }
    Ok(json)
}

/// Create a post through the REST API, as the block editor does: a POST of the
/// post's fields as JSON, with the logged in user's REST nonce (see
/// [`get_rest_nonce`]) in an `X-WP-Nonce` header.
///
/// Fails when WordPress refuses, for example a 403 `rest_cookie_invalid_nonce`
/// for a missing or stale nonce, or answers without a post. Returns the post
/// WordPress created, as JSON.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::{create_post, get_rest_nonce, Post};
///
/// transaction!(write);
///
/// async fn write(user: &mut GooseUser) -> TransactionResult {
///     // Any admin page has the nonce; the editor would load post-new.php.
///     let html = user.get("wp-admin/").await?.response?.text().await?;
///     let nonce = get_rest_nonce(&html).unwrap_or_default();
///     let post = Post::builder().title("Written by Goose").build();
///     let created = create_post(user, &nonce, &post).await?;
///     assert!(created["id"].as_u64().is_some());
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::rest_site("wp-json/wp/v2/posts", r#"{"title":"Written by Goose"}"#);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(write(&mut user));
/// # assert!(result.is_ok(), "doc example create_post: {}", support::failure(&result));
/// # assert_eq!(server.saves(), 1, "doc example create_post: posts saved");
/// ```
pub async fn create_post(
    user: &mut GooseUser,
    nonce: &str,
    post: &Post<'_>,
) -> Result<serde_json::Value, Box<TransactionError>> {
    save_post(user, nonce, post.url, post.url, post).await
}

/// Update a post through the REST API, as the block editor saves one: a POST of
/// the changed fields as JSON to the post's route, with the REST nonce in an
/// `X-WP-Nonce` header.
///
/// The request is named `<route>/{id}`, so updates of every post share one line
/// in the metrics. Fails as [`create_post`] does; returns the updated post.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::{get_rest_nonce, update_post, Post};
///
/// transaction!(edit);
///
/// async fn edit(user: &mut GooseUser) -> TransactionResult {
///     let html = user.get("wp-admin/").await?.response?.text().await?;
///     let nonce = get_rest_nonce(&html).unwrap_or_default();
///     let post = Post::builder().title("Hello world!").build();
///     let _updated = update_post(user, &nonce, 1, &post).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::rest_site("wp-json/wp/v2/posts/1", r#"{"title":"Hello world!"}"#);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(edit(&mut user));
/// # assert!(result.is_ok(), "doc example update_post: {}", support::failure(&result));
/// # assert_eq!(server.saves(), 1, "doc example update_post: posts saved");
/// ```
pub async fn update_post(
    user: &mut GooseUser,
    nonce: &str,
    id: u64,
    post: &Post<'_>,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let path = format!("{}/{}", post.url.trim_end_matches('/'), id);
    let name = format!("{}/{{id}}", post.url.trim_end_matches('/'));
    save_post(user, nonce, &path, &name, post).await
}

/// The URLs of a site's content, by kind, as [`discover`] found them.
///
/// Kinds are WordPress's own names: a post type (`post`, `page`, or a plugin's,
/// such as WooCommerce's `product`) or a taxonomy (`category`, `post_tag`,
/// `product_cat`). Each URL is a path on the site, such as `/hello-world/`.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::SiteContent;
///
/// // Absolute URLs, as sitemaps list them, are kept as paths on the site.
/// let content = SiteContent::from_urls(&[("post", "https://example.com/hello-world/")]);
/// assert_eq!(content.posts(), ["/hello-world/"]);
/// ```
#[derive(Clone, Debug, Default)]
pub struct SiteContent {
    urls: BTreeMap<String, Vec<String>>,
}
impl SiteContent {
    /// The URLs of one kind of content; empty when the site has none.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/hello-world/"), ("page", "/about/")]);
    /// assert_eq!(content.urls("post"), ["/hello-world/"]);
    /// assert!(content.urls("product").is_empty());
    /// ```
    pub fn urls(&self, kind: &str) -> &[String] {
        self.urls.get(kind).map_or(&[], Vec::as_slice)
    }

    /// The kinds of content found, in name order.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/hello-world/"), ("page", "/about/")]);
    /// assert_eq!(content.kinds().collect::<Vec<_>>(), ["page", "post"]);
    /// ```
    pub fn kinds(&self) -> impl Iterator<Item = &str> {
        self.urls.keys().map(String::as_str)
    }

    /// Posts.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/hello-world/"), ("page", "/about/")]);
    /// assert_eq!(content.posts(), ["/hello-world/"]);
    /// ```
    pub fn posts(&self) -> &[String] {
        self.urls("post")
    }

    /// Pages.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/hello-world/"), ("page", "/about/")]);
    /// assert_eq!(content.pages(), ["/about/"]);
    /// ```
    pub fn pages(&self) -> &[String] {
        self.urls("page")
    }

    /// Category archives.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("category", "/category/news/")]);
    /// assert_eq!(content.categories(), ["/category/news/"]);
    /// ```
    pub fn categories(&self) -> &[String] {
        self.urls("category")
    }

    /// Tag archives.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post_tag", "/tag/rust/")]);
    /// assert_eq!(content.tags(), ["/tag/rust/"]);
    /// ```
    pub fn tags(&self) -> &[String] {
        self.urls("post_tag")
    }

    /// A URL of one kind, picked at random; `None` when the site has none.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/a/"), ("post", "/b/")]);
    /// let post = content.random("post").unwrap();
    /// assert!(post == "/a/" || post == "/b/");
    /// assert_eq!(content.random("page"), None);
    /// ```
    pub fn random(&self, kind: &str) -> Option<&str> {
        self.urls(kind).choose(&mut rand::rng()).map(String::as_str)
    }

    /// Build content from `(kind, url)` pairs, for tests or a site whose URLs are
    /// already known. URLs keep their order within a kind.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::SiteContent;
    ///
    /// let content = SiteContent::from_urls(&[("post", "/a/"), ("post", "/b/")]);
    /// assert_eq!(content.posts(), ["/a/", "/b/"]);
    /// ```
    pub fn from_urls(urls: &[(&str, &str)]) -> SiteContent {
        let mut content = SiteContent::default();
        for (kind, url) in urls {
            content.add(kind, url.to_string());
        }
        content
    }

    // Add a URL, as a path on the site.
    fn add(&mut self, kind: &str, url: String) {
        self.urls
            .entry(kind.to_string())
            .or_default()
            .push(site_path(&url));
    }
}

// The path (and query) of a URL, so it is requested on the load test's host.
pub(crate) fn site_path(url: &str) -> String {
    match Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*://[^/]*")
        .unwrap()
        .find(url)
    {
        Some(origin) if origin.end() == url.len() => "/".to_string(),
        Some(origin) => url[origin.end()..].to_string(),
        None => url.to_string(),
    }
}

// Every <loc> in a sitemap, decoded.
fn sitemap_locations(xml: &str) -> Vec<String> {
    Regex::new(r"(?s)<loc>\s*(.*?)\s*</loc>")
        .unwrap()
        .captures_iter(xml)
        .map(|c| html_escape::decode_html_entities(&c[1]).to_string())
        .collect()
}

// The kind of content a core sub sitemap lists, from its name:
// wp-sitemap-posts-<post type>-<n>.xml, wp-sitemap-taxonomies-<taxonomy>-<n>.xml
// or wp-sitemap-users-<n>.xml.
fn sitemap_kind(url: &str) -> Option<String> {
    let re =
        Regex::new(r"wp-sitemap-(?:posts|taxonomies)-(.+)-\d+\.xml|wp-sitemap-(users)-\d+\.xml")
            .unwrap();
    let capture = re.captures(url)?;
    Some(
        match capture.get(1) {
            Some(kind) => kind.as_str(),
            None => "user",
        }
        .to_string(),
    )
}

/// Where [`discover`] reads a site's content from. For complete documentation,
/// refer to [`DiscoverBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Discover;
///
/// let discover: Discover = Discover::builder().build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::rest_list_site("wp-json/wp/v2/");
/// # let content = support::discover("Discover", &server, &discover);
/// # assert_eq!(content.pages(), ["/about/"], "doc example Discover: pages");
/// ```
#[derive(Clone, Debug)]
pub struct Discover<'a> {
    // The path of the core sitemap index.
    sitemap: &'a str,
    // The REST API route that lists content, used when there is no sitemap.
    rest: &'a str,
    // The REST routes to list, with the kind each lists.
    rest_kinds: &'a [(&'a str, &'a str)],
    // At most this many URLs of each kind.
    max_per_kind: usize,
}
impl<'a> Discover<'a> {
    /// Convenience function to bring [`DiscoverBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// let _discover = Discover::builder().max_per_kind(10).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::sitemap_site("wp-sitemap.xml");
    /// # let content = support::discover("Discover::builder", &server, &_discover);
    /// # assert_eq!(content.posts().len(), 10, "doc example Discover::builder: posts");
    /// ```
    pub fn builder() -> DiscoverBuilder<'a> {
        DiscoverBuilder::new()
    }
}

/// Used to build a [`Discover`] object, necessary to invoke the [`discover`] function.
///
/// # Example
/// ```rust
/// use goose_eggs::wordpress::Discover;
///
/// // A site with its sitemap at the default path, up to 50 URLs of each kind.
/// let _discover = Discover::builder()
///     .sitemap("wp-sitemap.xml")
///     .max_per_kind(50)
///     .build();
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::sitemap_site("wp-sitemap.xml");
/// # let content = support::discover("DiscoverBuilder", &server, &_discover);
/// # assert_eq!(content.posts().len(), 50, "doc example DiscoverBuilder: posts");
/// ```
pub struct DiscoverBuilder<'a> {
    sitemap: &'a str,
    rest: &'a str,
    rest_kinds: &'a [(&'a str, &'a str)],
    max_per_kind: usize,
}
impl<'a> DiscoverBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to the core sitemap, WordPress 5.5 and later.
            sitemap: "wp-sitemap.xml",
            // Defaults to the core REST API.
            rest: "wp-json/wp/v2/",
            // Defaults to the core content the sitemap also lists.
            rest_kinds: &[
                ("posts", "post"),
                ("pages", "page"),
                ("categories", "category"),
                ("tags", "post_tag"),
            ],
            // Defaults to 100 URLs of each kind, one REST page each.
            max_per_kind: 100,
        }
    }

    /// The path of the core sitemap index.
    ///
    /// Defaults to `wp-sitemap.xml`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// let _discover = Discover::builder().sitemap("blog/wp-sitemap.xml").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::sitemap_site("blog/wp-sitemap.xml");
    /// # let content = support::discover("DiscoverBuilder::sitemap", &server, &_discover);
    /// # assert!(!content.posts().is_empty(), "doc example DiscoverBuilder::sitemap: posts");
    /// ```
    pub fn sitemap(mut self, sitemap: &'a str) -> Self {
        self.sitemap = sitemap;
        self
    }

    /// The REST API route [`discover`] lists content from when the site has no
    /// sitemap (a plugin or the `wp_sitemaps_enabled` filter turned it off).
    ///
    /// Defaults to `wp-json/wp/v2/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// // A site without pretty permalinks serves the REST API at ?rest_route=.
    /// let _discover = Discover::builder().rest("api/wp/v2/").build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_list_site("api/wp/v2/");
    /// # let content = support::discover("DiscoverBuilder::rest", &server, &_discover);
    /// # assert!(!content.posts().is_empty(), "doc example DiscoverBuilder::rest: posts");
    /// ```
    pub fn rest(mut self, rest: &'a str) -> Self {
        self.rest = rest;
        self
    }

    /// The REST routes to list when there is no sitemap, each with the kind of
    /// content it lists, for example `&[("posts", "post"), ("products", "product")]`.
    ///
    /// Defaults to posts, pages, categories and tags.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// // Only posts.
    /// let _discover = Discover::builder().rest_kinds(&[("posts", "post")]).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::rest_list_site("wp-json/wp/v2/");
    /// # let content = support::discover("DiscoverBuilder::rest_kinds", &server, &_discover);
    /// # assert_eq!(content.kinds().collect::<Vec<_>>(), ["post"], "doc example DiscoverBuilder::rest_kinds: kinds");
    /// ```
    pub fn rest_kinds(mut self, rest_kinds: &'a [(&'a str, &'a str)]) -> Self {
        self.rest_kinds = rest_kinds;
        self
    }

    /// Read at most this many URLs of each kind, which bounds how many requests
    /// discovery makes on a large site.
    ///
    /// Defaults to 100.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// let _discover = Discover::builder().max_per_kind(5).build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::sitemap_site("wp-sitemap.xml");
    /// # let content = support::discover("DiscoverBuilder::max_per_kind", &server, &_discover);
    /// # assert_eq!(content.posts().len(), 5, "doc example DiscoverBuilder::max_per_kind: posts");
    /// ```
    pub fn max_per_kind(mut self, max_per_kind: usize) -> Self {
        self.max_per_kind = max_per_kind;
        self
    }

    /// Build the [`Discover`] object which is then passed to the [`discover`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::wordpress::Discover;
    ///
    /// let _discover = Discover::builder().build();
    /// # mod support { include!("../tests/doc/wordpress.rs"); }
    /// # let server = support::sitemap_site("wp-sitemap.xml");
    /// # let content = support::discover("DiscoverBuilder::build", &server, &_discover);
    /// # assert!(!content.pages().is_empty(), "doc example DiscoverBuilder::build: pages");
    /// ```
    pub fn build(self) -> Discover<'a> {
        let Self {
            sitemap,
            rest,
            rest_kinds,
            max_per_kind,
        } = self;
        Discover {
            sitemap,
            rest,
            rest_kinds,
            max_per_kind,
        }
    }
}

/// Find a site's content, so a load test can request its real URLs at random.
///
/// Reads the core sitemap index (WordPress 5.5 and later) and the sub sitemaps
/// it lists, up to [`DiscoverBuilder::max_per_kind`] URLs of each kind. When the
/// site has no sitemap, lists the content through the public REST API instead,
/// up to 100 a page, following `X-WP-TotalPages`.
///
/// The sitemap index is requested outside Goose's metrics, so a site without one
/// records no error; every other request is counted. Discovery makes several
/// requests, so run it once per load test and share the result, for example in
/// a [`std::sync::OnceLock`] filled by the first user to start.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::wordpress::{discover, Discover, SiteContent};
/// use std::sync::OnceLock;
///
/// static CONTENT: OnceLock<SiteContent> = OnceLock::new();
///
/// transaction!(find_content).set_on_start();
/// transaction!(read_a_post);
///
/// async fn find_content(user: &mut GooseUser) -> TransactionResult {
///     if CONTENT.get().is_none() {
///         let content = discover(user, &Discover::builder().build()).await?;
///         let _ = CONTENT.set(content);
///     }
///     Ok(())
/// }
///
/// async fn read_a_post(user: &mut GooseUser) -> TransactionResult {
///     if let Some(post) = CONTENT.get().and_then(|c| c.random("post")) {
///         let _goose = user.get_named(post, "post").await?;
///     }
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/wordpress.rs"); }
/// # let server = support::sitemap_site("wp-sitemap.xml");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(async { find_content(&mut user).await?; read_a_post(&mut user).await });
/// # assert!(result.is_ok(), "doc example discover: {}", support::failure(&result));
/// # assert_eq!(CONTENT.get().map(|c| c.posts().len()), Some(57), "doc example discover: posts");
/// ```
pub async fn discover(
    user: &mut GooseUser,
    discover: &Discover<'_>,
) -> Result<SiteContent, Box<TransactionError>> {
    let mut content = SiteContent::default();
    let cap = discover.max_per_kind;

    // The sitemap index, read without a metric so its absence is no error.
    let url = user.build_url(discover.sitemap)?;
    let index = match user.client.get(&url).send().await {
        Ok(response) if response.status().is_success() => response
            .text()
            .await
            .ok()
            .filter(|xml| xml.contains("<sitemapindex")),
        _ => None,
    };

    if let Some(index) = index {
        for sitemap in sitemap_locations(&index) {
            let kind = match sitemap_kind(&sitemap) {
                Some(kind) => kind,
                None => continue,
            };
            if content.urls(&kind).len() >= cap {
                continue;
            }
            let goose = user.get(&site_path(&sitemap)).await?;
            request_ok(&goose)?;
            let (_, xml) = read_response(goose).await;
            for url in sitemap_locations(&xml) {
                if content.urls(&kind).len() >= cap {
                    break;
                }
                content.add(&kind, url);
            }
        }
        return Ok(content);
    }

    // No sitemap: list each kind through the REST API, with one page size
    // throughout so the pages line up.
    let per_page = cap.min(100);
    for (route, kind) in discover.rest_kinds {
        let mut page = 1;
        while content.urls(kind).len() < cap {
            let path = format!(
                "{}{}?per_page={}&page={}&_fields=link",
                discover.rest, route, per_page, page
            );
            let name = format!("{}{}", discover.rest, route);
            let goose = get_named(user, &path, &name).await?;
            let (headers, json) = json_response(user, goose).await?;
            let links: Vec<String> = json
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.get("link").and_then(|l| l.as_str()))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let found = links.len();
            for link in links {
                if content.urls(kind).len() >= cap {
                    break;
                }
                content.add(kind, link);
            }
            let pages = header_value(&headers, "X-WP-TotalPages")
                .and_then(|p| p.parse::<usize>().ok())
                .unwrap_or(1);
            if found == 0 || page >= pages {
                break;
            }
            page += 1;
        }
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pages captured from the theme test data site by
    // examples/wordpress-sites/capture-fixtures.sh.
    const LOGIN_PAGE: &str = include_str!("../tests/fixtures/wordpress/login.html");
    const LOGIN_FAILED: &str = include_str!("../tests/fixtures/wordpress/login-failed.html");
    const DASHBOARD: &str = include_str!("../tests/fixtures/wordpress/dashboard.html");
    const COMMENT_ANONYMOUS: &str =
        include_str!("../tests/fixtures/wordpress/comment-form-anonymous.html");
    const COMMENT_LOGGED_IN: &str =
        include_str!("../tests/fixtures/wordpress/comment-form-logged-in.html");
    const SITEMAP: &str = include_str!("../tests/fixtures/wordpress/sitemap.xml");
    const SITEMAP_POSTS: &str =
        include_str!("../tests/fixtures/wordpress/sitemap-posts-post-1.xml");
    const CLASSIC_CHECKOUT: &str =
        include_str!("../tests/fixtures/woocommerce/classic-checkout.html");

    #[test]
    fn attribute_cases() {
        // (case, tag, attribute, expected)
        let cases = [
            (
                "double quoted",
                r#"<input name="a" value="b">"#,
                "value",
                Some("b"),
            ),
            (
                "single quoted",
                "<input name='a' value='b'>",
                "value",
                Some("b"),
            ),
            ("unquoted", "<input name=a value=b>", "value", Some("b")),
            ("no value", "<input checked name=a>", "checked", Some("")),
            (
                "last, self closing",
                r#"<input name="a" checked/>"#,
                "checked",
                Some(""),
            ),
            (
                "entities decoded",
                r#"<a href="/x?a=1&#038;b=2">"#,
                "href",
                Some("/x?a=1&b=2"),
            ),
            (
                "newlines",
                "<input\n\tname=\"a\"\n\tvalue=\"b\"\n/>",
                "value",
                Some("b"),
            ),
            (
                "upper case",
                r#"<INPUT NAME="a" VALUE="b">"#,
                "value",
                Some("b"),
            ),
            (
                "name is not a suffix",
                r#"<input data-value="x" value="b">"#,
                "value",
                Some("b"),
            ),
            ("only a suffix", r#"<input data-value="x">"#, "value", None),
            ("missing", r#"<input name="a">"#, "value", None),
        ];
        for (case, tag, name, expected) in cases {
            assert_eq!(
                attribute(tag, name).as_deref(),
                expected,
                "attribute case {}",
                case
            );
        }
    }

    #[test]
    fn get_form_cases() {
        // (case, html, form name, expected form)
        let cases = [
            (
                "by id",
                r#"<form id="a" method="post"><p>A</p></form>"#,
                "a",
                "<p>A</p>",
            ),
            (
                "by name",
                r#"<form method="post" name="b"><p>B</p></form>"#,
                "b",
                "<p>B</p>",
            ),
            (
                "second of two",
                r#"<form id="a">A</form><form id="b">B</form>"#,
                "b",
                "B",
            ),
            (
                "longer name first",
                r#"<form id="a-extra">X</form><form id="a">Y</form>"#,
                "a",
                "Y",
            ),
            (
                "upper case end",
                "<form id=\"a\">\nA\n</FORM>",
                "a",
                "\nA\n",
            ),
            ("unclosed", r#"<form id="a">A"#, "a", "A"),
            ("missing", r#"<form id="a">A</form>"#, "b", ""),
        ];
        for (case, html, name, expected) in cases {
            assert_eq!(get_form(html, name), expected, "get_form case {}", case);
        }

        // On a real post page the comment form is not the search form before it.
        let form = get_form(COMMENT_ANONYMOUS, "commentform");
        assert!(
            form.contains("comment_post_ID") && !form.contains("</form>"),
            "get_form case wordpress comment form: {}",
            form
        );
        let form = get_form(LOGIN_PAGE, "loginform");
        assert!(
            form.contains(r#"name="pwd""#),
            "get_form case wordpress login form"
        );
        assert_eq!(
            get_form_by_class(CLASSIC_CHECKOUT, "woocommerce-form-coupon"),
            get_form(CLASSIC_CHECKOUT, "woocommerce-checkout-form-coupon"),
            "get_form case by class"
        );
        assert_eq!(
            get_form_by_class(r#"<form class="a-b c">X</form>"#, "a"),
            "",
            "get_form case class is a whole word"
        );
    }

    #[test]
    fn form_controls_cases() {
        let form = concat!(
            r#"<input type="hidden" name="hidden" value="h" />"#,
            r#"<input type="text" name="text" />"#,
            r#"<input type="checkbox" name="unchecked" value="u" />"#,
            r#"<input type="checkbox" name="checked" value="c" checked />"#,
            r#"<input type="checkbox" name="checked_no_value" checked="checked" />"#,
            r#"<input type="radio" name="radio" value="first" />"#,
            r#"<input type="radio" name="radio" value="second" checked='checked' />"#,
            r#"<input type="submit" name="submit" value="Go" />"#,
            r#"<input type="button" name="button" value="B" />"#,
            r#"<input type="image" name="image" src="x.png" />"#,
            r#"<input type="text" value="no name" />"#,
            r#"<textarea name="area" rows="3">Line &amp; more</textarea>"#,
            "<select name=\"chosen\"><option value=\"a\">A</option>\n<option value=\"b\" selected>B</option></select>",
            r#"<select name="first"><option value="x">X</option><option value="y">Y</option></select>"#,
            r#"<select name="empty"></select>"#,
        );
        let expected: Vec<(String, String)> = [
            ("hidden", "h"),
            ("text", ""),
            ("checked", "c"),
            ("checked_no_value", "on"),
            ("radio", "second"),
            ("area", "Line & more"),
            ("chosen", "b"),
            ("first", "x"),
        ]
        .iter()
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect();
        assert_eq!(
            form_controls(form),
            expected,
            "form_controls case all kinds"
        );

        // WooCommerce's classic checkout: the chosen shipping method and payment
        // method come from the form, the country from its select.
        let controls = form_controls(&get_form(CLASSIC_CHECKOUT, "checkout"));
        let value = |name: &str| {
            controls
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(
            value("payment_method"),
            Some("cod"),
            "form_controls case checkout payment"
        );
        assert_eq!(
            value("shipping_method[0]"),
            Some("free_shipping:1"),
            "form_controls case checkout shipping"
        );
        assert_eq!(
            value("billing_country"),
            Some("US"),
            "form_controls case checkout country"
        );
        assert_eq!(
            value("ship_to_different_address"),
            None,
            "form_controls case unchecked"
        );
    }

    #[test]
    fn get_form_value_cases() {
        let form = get_form(COMMENT_ANONYMOUS, "commentform");
        // (case, element name, expected value)
        let cases = [
            ("value after id", "comment_parent", Some("0")),
            ("value before id", "comment_post_ID", Some("1")),
            ("empty", "author", Some("")),
            ("textarea", "comment", Some("")),
            ("submit is not sent", "submit", None),
            ("missing", "_wp_unfiltered_html_comment", None),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                get_form_value(&form, name).as_deref(),
                expected,
                "get_form_value case {}",
                case
            );
        }
        let form = get_form(COMMENT_LOGGED_IN, "commentform");
        assert!(
            get_form_value(&form, "_wp_unfiltered_html_comment_disabled")
                .is_some_and(|nonce| nonce.len() == 10),
            "get_form_value case logged in unfiltered html nonce"
        );
        assert_eq!(
            get_form_value(&form, "author"),
            None,
            "get_form_value case logged in"
        );
    }

    #[test]
    fn get_form_values_cases() {
        let form = get_form(LOGIN_PAGE, "loginform");
        let values = get_form_values(&form, &["log", "testcookie", "redirect_to", "missing"]);
        assert_eq!(values.len(), 3, "get_form_values case count");
        assert_eq!(
            values.get("log").map(String::as_str),
            Some(""),
            "get_form_values case empty"
        );
        assert_eq!(
            values.get("testcookie").map(String::as_str),
            Some("1"),
            "get_form_values case testcookie"
        );
        assert_eq!(
            values.get("redirect_to").map(String::as_str),
            Some("/wp-admin/"),
            "get_form_values case redirect_to"
        );
    }

    #[test]
    fn get_nonce_cases() {
        let html = concat!(
            r#"<input type="hidden" id="_wpnonce" name="_wpnonce" value="abc123" />"#,
            r#"<input type="hidden" name="empty-nonce" value="" />"#,
        );
        // (case, field name, expected)
        let cases = [
            ("wpnonce", "_wpnonce", Some("abc123")),
            ("empty", "empty-nonce", None),
            ("missing", "other", None),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                get_nonce(html, name).as_deref(),
                expected,
                "get_nonce case {}",
                case
            );
        }
    }

    #[test]
    fn get_rest_nonce_cases() {
        // (case, html, expected)
        let cases = [
            (
                "middleware",
                r#"wp.apiFetch.createNonceMiddleware( "aaa111" );"#,
                Some("aaa111"),
            ),
            (
                "middleware, single quotes, no spaces",
                "createNonceMiddleware('bbb222')",
                Some("bbb222"),
            ),
            (
                "wpApiSettings",
                r#"var wpApiSettings = {"root":"/wp-json/","nonce":"ccc333"};"#,
                Some("ccc333"),
            ),
            (
                "another nonce is not the REST nonce",
                r#"var other = {"nonce":"ddd444"};"#,
                None,
            ),
            ("missing", "<html></html>", None),
        ];
        for (case, html, expected) in cases {
            assert_eq!(
                get_rest_nonce(html).as_deref(),
                expected,
                "get_rest_nonce case {}",
                case
            );
        }
        assert!(
            get_rest_nonce(DASHBOARD).is_some_and(|n| n.len() == 10),
            "get_rest_nonce case dashboard"
        );
    }

    #[test]
    fn get_log_out_url_cases() {
        // (case, html, expected)
        let cases = [
            (
                "admin bar",
                r#"<a class="ab-item" href="/wp-login.php?action=logout&#038;_wpnonce=abc">Log Out</a>"#,
                Some("/wp-login.php?action=logout&_wpnonce=abc"),
            ),
            (
                "single quotes, absolute",
                "<a href='https://example.com/wp-login.php?action=logout&amp;_wpnonce=def'>",
                Some("https://example.com/wp-login.php?action=logout&_wpnonce=def"),
            ),
            (
                "log in link is not a log out link",
                r#"<a href="/wp-login.php?action=lostpassword">"#,
                None,
            ),
        ];
        for (case, html, expected) in cases {
            assert_eq!(
                get_log_out_url(html).as_deref(),
                expected,
                "get_log_out_url case {}",
                case
            );
        }
        assert!(
            get_log_out_url(DASHBOARD).is_some_and(|url| url.contains("&_wpnonce=")),
            "get_log_out_url case dashboard"
        );
    }

    #[test]
    fn plain_text_cases() {
        assert_eq!(
            plain_text("<p><strong>Error:</strong>\n  The password &amp; more.</p>"),
            "Error: The password & more.",
            "plain_text case tags and entities"
        );
        let error = Regex::new(r#"(?s)id="login_error"[^>]*>(.*?)</div>"#)
            .unwrap()
            .captures(LOGIN_FAILED)
            .map(|c| plain_text(&c[1]))
            .unwrap();
        assert!(
            error.starts_with("Error: The password you entered for the username"),
            "plain_text case wordpress login error: {}",
            error
        );
    }

    #[test]
    fn query_encode_cases() {
        // (case, value, expected)
        let cases = [
            ("unreserved", "aZ09-._~", "aZ09-._~"),
            ("space", "foo bar", "foo+bar"),
            ("reserved", "a&b=c?d/e+f#", "a%26b%3Dc%3Fd%2Fe%2Bf%23"),
            ("utf-8", "café", "caf%C3%A9"),
        ];
        for (case, value, expected) in cases {
            assert_eq!(query_encode(value), expected, "query_encode case {}", case);
        }
    }

    #[test]
    fn site_path_cases() {
        // (case, url, expected)
        let cases = [
            (
                "absolute",
                "http://localhost:18081/hello-world/",
                "/hello-world/",
            ),
            ("https with query", "https://example.com/?p=1", "/?p=1"),
            ("host only", "https://example.com", "/"),
            ("already a path", "/about/", "/about/"),
        ];
        for (case, url, expected) in cases {
            assert_eq!(site_path(url), expected, "site_path case {}", case);
        }
    }

    #[test]
    fn sitemap_cases() {
        let sitemaps = sitemap_locations(SITEMAP);
        assert_eq!(
            sitemaps.len(),
            6,
            "sitemap case index entries: {:?}",
            sitemaps
        );
        let kinds: Vec<Option<String>> = sitemaps.iter().map(|s| sitemap_kind(s)).collect();
        let expected: Vec<Option<String>> = [
            "post",
            "page",
            "category",
            "post_tag",
            "post_format",
            "user",
        ]
        .iter()
        .map(|k| Some(k.to_string()))
        .collect();
        assert_eq!(kinds, expected, "sitemap case kinds");
        assert_eq!(
            sitemap_kind("https://example.com/wp-sitemap-posts-product-2.xml").as_deref(),
            Some("product"),
            "sitemap case plugin post type"
        );
        assert_eq!(
            sitemap_kind("https://example.com/sitemap_index.xml"),
            None,
            "sitemap case not a core sitemap"
        );
        let posts = sitemap_locations(SITEMAP_POSTS);
        assert_eq!(posts.len(), 57, "sitemap case posts");
        assert_eq!(posts[0], "/hello-world/", "sitemap case first post");
        assert_eq!(
            sitemap_locations("<url><loc>\n https://e.com/?a=1&amp;b=2 \n</loc></url>"),
            ["https://e.com/?a=1&b=2"],
            "sitemap case decoded and trimmed"
        );
    }

    #[test]
    fn site_content_cases() {
        let content = SiteContent::from_urls(&[
            ("post", "https://example.com/a/"),
            ("post", "/b/"),
            ("page", "/about/"),
            ("category", "/category/news/"),
            ("post_tag", "/tag/rust/"),
        ]);
        assert_eq!(content.posts(), ["/a/", "/b/"], "site_content case posts");
        assert_eq!(content.pages(), ["/about/"], "site_content case pages");
        assert_eq!(
            content.categories(),
            ["/category/news/"],
            "site_content case categories"
        );
        assert_eq!(content.tags(), ["/tag/rust/"], "site_content case tags");
        assert!(
            content.urls("product").is_empty(),
            "site_content case missing kind"
        );
        assert_eq!(
            content.kinds().collect::<Vec<_>>(),
            ["category", "page", "post", "post_tag"],
            "site_content case kinds"
        );
        let mut picked = std::collections::HashSet::new();
        for _ in 0..200 {
            picked.insert(content.random("post").unwrap().to_string());
        }
        assert_eq!(picked.len(), 2, "site_content case random picks every post");
        assert_eq!(
            content.random("product"),
            None,
            "site_content case random of none"
        );
    }

    #[test]
    fn login_builder_defaults() {
        let login = Login::builder().build();
        assert_eq!(login.username, "username", "Login default username");
        assert_eq!(login.password, "password", "Login default password");
        assert_eq!(login.url, "wp-login.php", "Login default url");
        assert_eq!(login.redirect_to, "", "Login default redirect_to");
        assert!(
            login.log_in_page_validation.is_none(),
            "Login default log in page validation"
        );
        assert!(
            login.logged_in_page_validation.is_none(),
            "Login default logged in page validation"
        );
    }

    #[test]
    fn search_params_builder_defaults() {
        let params = SearchParams::builder().build();
        assert_eq!(params.keys, "", "SearchParams default keys");
        assert_eq!(params.url, "", "SearchParams default url");
        assert_eq!(params.post_type, None, "SearchParams default post_type");
        assert!(
            params.results_page_validation.is_none(),
            "SearchParams default validation"
        );
    }

    #[test]
    fn comment_builder_defaults() {
        let comment = Comment::builder().build();
        assert_eq!(comment.post_url, "", "Comment default post_url");
        assert_eq!(comment.text, None, "Comment default text");
        assert_eq!(comment.author, "Goose", "Comment default author");
        assert_eq!(comment.email, "goose@example.com", "Comment default email");
        assert_eq!(comment.url, "wp-comments-post.php", "Comment default url");
        assert_eq!(comment.parent, None, "Comment default parent");
        assert!(
            comment.post_page_validation.is_none(),
            "Comment default post page validation"
        );
        assert!(
            comment.commented_page_validation.is_none(),
            "Comment default commented page validation"
        );
    }

    #[test]
    fn post_builder_defaults() {
        let post = Post::builder().build();
        assert_eq!(post.url, "wp-json/wp/v2/posts", "Post default url");
        assert!(post.fields.is_empty(), "Post default fields");
        let post = Post::builder().title("a").title("b").build();
        assert_eq!(
            post.fields.get("title"),
            Some(&serde_json::json!("b")),
            "Post last title wins"
        );
    }

    #[test]
    fn discover_builder_defaults() {
        let discover = Discover::builder().build();
        assert_eq!(
            discover.sitemap, "wp-sitemap.xml",
            "Discover default sitemap"
        );
        assert_eq!(discover.rest, "wp-json/wp/v2/", "Discover default rest");
        assert_eq!(
            discover.rest_kinds,
            [
                ("posts", "post"),
                ("pages", "page"),
                ("categories", "category"),
                ("tags", "post_tag"),
            ],
            "Discover default rest_kinds"
        );
        assert_eq!(discover.max_per_kind, 100, "Discover default max_per_kind");
    }
}
