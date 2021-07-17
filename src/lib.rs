//! Goose Eggs contains helpful functions and structures for writing
//! [`Goose`](https://docs.rs/goose) load tests.
use goose::goose::GooseResponse;
use goose::prelude::*;
use log::info;
use reqwest::header::HeaderMap;

use regex::Regex;

/// Define one or more items to be validated in a web page response.
///
/// This structure is passed to [`validate_and_load_static_assets`].
///
/// # Example
/// ```rust
/// use goose_eggs::Validate;
///
/// fn example() {
///     let _validate = Validate {
///         // Don't do any extra validation of the status code.
///         status: None,
///         // Be sure the expected title is on the page.
///         title: Some("my page"),
///         // Be sure both of the following strings are found on the page.
///         texts: vec!["foo", r#"<a href="bar">"#],
///         // Don't do any validation of the headers.
///         headers: vec![],
///     };
/// }
#[derive(Clone, Debug)]
pub struct Validate<'a> {
    /// If provided, validate the response status code.
    pub status: Option<u16>,
    /// If provided, validate the response title.
    pub title: Option<&'a str>,
    /// If provided, validate that the provided texts are found on the page.
    pub texts: Vec<&'a str>,
    /// If provided, validate that the provided headers were set in the HTTP response.
    pub headers: Vec<&'a Header<'a>>,
}

/// Used to validate that headers are included in the server response.
///
/// # Example
/// ```rust
/// use goose_eggs::Header;
///
/// fn example() {
///     let _cookie = Header {
///         // Validate that the "x-varnish" header is set.
///         name: "x-varnish",
///         // Don't further validate the contents of the header.
///         value: None,
///     };
/// }

#[derive(Clone, Debug)]
pub struct Header<'a> {
    /// The name of the header to validate, required.
    pub name: &'a str,
    /// The value of the header to validate, optional.
    pub value: Option<&'a str>,
}

/// Returns a [`bool`] indicating whether or not the title (case insensitive) is
/// found within the html.
///
/// A valid title starts with `<title>foo` where `foo` is the expected title text.
/// Returns [`true`] if the expected title is found, otherwise returns [`false`].
///
/// This function is case insensitive, if a title of "foo" is specified it will
/// match "foo" or "Foo" or "FOO".
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::valid_title;
///
/// task!(validate_title).set_on_start();
///
/// async fn validate_title(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             match response.text().await {
///                 Ok(html) => {
///                     let title = "example";
///                     if !valid_title(&html, title) {
///                         return user.set_failure(
///                             &format!("{}: title not found: {}", goose.request.url, title),
///                             &mut goose.request,
///                             Some(&headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.url, e),
///                         &mut goose.request,
///                         Some(&headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.url, e),
///                 &mut goose.request,
///                 None,
///                 None,
///             );
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub fn valid_title(html: &str, title: &str) -> bool {
    html.to_ascii_lowercase()
        .contains(&("<title>".to_string() + title.to_ascii_lowercase().as_str()))
}

/// Returns a [`bool`] indicating whether or not an arbitrary str (case sensitive) is found
/// within the html.
///
/// Returns [`true`] if the expected str is found, otherwise returns [`false`].
///
/// This function is case sensitive, if the text "foo" is specified it will only match "foo",
/// not "Foo" or "FOO".
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::valid_text;
///
/// task!(validate_text).set_on_start();
///
/// async fn validate_text(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             match response.text().await {
///                 Ok(html) => {
///                     let text = r#"<code class="language-console">$ cargo new hello_world --bin"#;
///                     if !valid_text(&html, text) {
///                         return user.set_failure(
///                             &format!("{}: text not found: {}", goose.request.url, text),
///                             &mut goose.request,
///                             Some(&headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.url, e),
///                         &mut goose.request,
///                         Some(&headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.url, e),
///                 &mut goose.request,
///                 None,
///                 None,
///             );
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub fn valid_text(html: &str, text: &str) -> bool {
    html.contains(text)
}

/// Returns a [`bool`] indicating whether or not a header was set in the server Response.
///
/// Returns [`true`] if the expected header was set, otherwise returns [`false`].
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{header_is_set, Header};
///
/// task!(validate_header).set_on_start();
///
/// async fn validate_header(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             if !header_is_set(headers, &Header{ name: "server", value: None }) {
///                 return user.set_failure(
///                     &format!("{}: header not found: {}", goose.request.url, "server"),
///                     &mut goose.request,
///                     Some(&headers),
///                     None,
///                 );
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.url, e),
///                 &mut goose.request,
///                 None,
///                 None,
///             );
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub fn header_is_set(headers: &HeaderMap, header: &Header) -> bool {
    headers.contains_key(header.name)
}

/// Returns a [`bool`] indicating whether or not a header contains an expected value.
///
/// Returns [`true`] if the expected value was found, otherwise returns [`false`].
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{valid_header_value, Header};
///
/// task!(validate_header_value).set_on_start();
///
/// async fn validate_header_value(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             if !valid_header_value(headers, &Header{ name: "server", value: Some("nginx") }) {
///                 return user.set_failure(
///                     &format!("{}: server header value not correct: {}", goose.request.url, "nginx"),
///                     &mut goose.request,
///                     Some(&headers),
///                     None,
///                 );
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.url, e),
///                 &mut goose.request,
///                 None,
///                 None,
///             );
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub fn valid_header_value(headers: &HeaderMap, header: &Header) -> bool {
    if header_is_set(headers, header) {
        if let Some(value_to_validate) = header.value {
            let header_value = match headers.get(header.name) {
                // Extract the value of the header and try to convert to a &str.
                Some(v) => v.to_str().unwrap_or(""),
                None => "",
            };
            // Check if the desired value is in the header.
            if header_value.contains(value_to_validate) {
                true
            } else {
                // Provide some extra debug.
                info!(
                    r#"header does not contain expected value: "{}: {}""#,
                    header.name, header_value
                );
                false
            }
        } else {
            false
        }
    } else {
        info!("header ({}) not set", header.name);
        false
    }
}

/// Extract and load all local static elements from the the provided html.
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::load_static_elements;
///
/// task!(load_page_and_static_elements).set_on_start();
///
/// async fn load_page_and_static_elements(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             match response.text().await {
///                 Ok(html) => {
///                     // Load all static elements on page.
///                     load_static_elements(user, &html);
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.url, e),
///                         &mut goose.request,
///                         Some(&headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.url, e),
///                 &mut goose.request,
///                 None,
///                 None,
///             );
///         }
///     }
///
///     Ok(())
/// }
/// ```
pub async fn load_static_elements(user: &GooseUser, html: &str) {
    // Use a regular expression to find all src=<foo> in the HTML, where foo
    // is the URL to image and js assets.
    // @TODO: parse HTML5 srcset= also
    let image = Regex::new(r#"src="(.*?)""#).unwrap();
    let mut urls = Vec::new();
    for url in image.captures_iter(&html) {
        if url[1].starts_with("/sites") || url[1].starts_with("/core") {
            urls.push(url[1].to_string());
        }
    }

    // Use a regular expression to find all href=<foo> in the HTML, where foo
    // is the URL to css assets.
    let css = Regex::new(r#"href="(/sites/default/files/css/.*?)""#).unwrap();
    for url in css.captures_iter(&html) {
        urls.push(url[1].to_string());
    }

    // Load all the static assets found on the page.
    for asset in &urls {
        let _ = user.get_named(asset, "static asset").await;
    }
}

/// Validate the HTML response then extract and load all static elements on the page.
///
/// What is validated is defined with the [`Validate`] structure.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{validate_and_load_static_assets, Validate};
///
/// task!(load_page).set_on_start();
///
/// async fn load_page(user: &GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///     validate_and_load_static_assets(
///         user,
///         goose,
///         &Validate {
///             // Don't do any extra validation of the status code.
///             status: None,
///             // Be sure the expected title is on the page.
///             title: Some("my page"),
///             // Be sure both of the following strings are found on the page.
///             texts: vec!["foo", r#"<a href="bar">"#],
///             // Don't do any validateion of headers.
///             headers: vec![],
///         },
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub async fn validate_and_load_static_assets<'a>(
    user: &GooseUser,
    mut goose: GooseResponse,
    validate: &'a Validate<'a>,
) -> GooseTaskResult {
    match goose.response {
        Ok(response) => {
            // Copy the headers so we have them for logging if there are errors.
            let headers = &response.headers().clone();
            let response_status = response.status();
            match response.text().await {
                Ok(html) => {
                    // Validate status code if defined.
                    if let Some(status) = validate.status {
                        if response_status != status {
                            return user.set_failure(
                                &format!(
                                    "{}: response status != {}]: {}",
                                    goose.request.url, status, response_status
                                ),
                                &mut goose.request,
                                Some(&headers),
                                Some(&html),
                            );
                        }
                    }
                    // Validate title if defined.
                    if let Some(title) = validate.title {
                        if !valid_title(&html, &title) {
                            return user.set_failure(
                                &format!("{}: title not found: {}", goose.request.url, title),
                                &mut goose.request,
                                Some(&headers),
                                Some(&html),
                            );
                        }
                    }
                    // Validate texts in body if defined.
                    for text in &validate.texts {
                        if !valid_text(&html, text) {
                            return user.set_failure(
                                &format!("{}: text not found on page: {}", goose.request.url, text),
                                &mut goose.request,
                                Some(&headers),
                                Some(&html),
                            );
                        }
                    }
                    // Validate headers if defined.
                    for header in &validate.headers {
                        if !header_is_set(headers, header) {
                            return user.set_failure(
                                &format!(
                                    "{}: header not included in response: {:?}",
                                    goose.request.url, header
                                ),
                                &mut goose.request,
                                Some(&headers),
                                Some(&html),
                            );
                        }
                        if let Some(h) = header.value {
                            if !valid_header_value(headers, header) {
                                return user.set_failure(
                                    &format!(
                                        "{}: header does not contain expected value: {:?}",
                                        goose.request.url, h
                                    ),
                                    &mut goose.request,
                                    Some(&headers),
                                    Some(&html),
                                );
                            }
                        }
                    }
                    load_static_elements(user, &html).await;
                }
                Err(e) => {
                    return user.set_failure(
                        &format!("{}: failed to parse page: {}", goose.request.url, e),
                        &mut goose.request,
                        Some(&headers),
                        None,
                    );
                }
            }
        }
        Err(e) => {
            return user.set_failure(
                &format!("{}: no response from server: {}", goose.request.url, e),
                &mut goose.request,
                None,
                None,
            );
        }
    }

    Ok(())
}
