//! Goose Eggs are helpful in writing [`Goose`](https://book.goose.rs/) load tests.
//!
//! ## Example
//! The [Umami example](https://github.com/tag1consulting/goose/tree/main/examples/umami)
//! included with Goose has been [converted to use the Goose Eggs library](https://github.com/tag1consulting/goose-eggs/tree/main/examples/umami)
//! and serves as a useful example on how to leverage it when writing load tests.

use goose::goose::GooseResponse;
use goose::prelude::*;
use log::info;
use regex::Regex;
use reqwest::header::HeaderMap;

pub mod drupal;
pub mod text;

/// Define one or more items to be validated in a web page response. For complete
/// documentation, refer to [`ValidateBuilder`].
///
/// This structure is passed to [`validate_and_load_static_assets`].
#[derive(Clone, Debug)]
pub struct Validate<'a> {
    /// Optionally validate the response status code.
    status: Option<u16>,
    /// Optionally validate the response title.
    title: Option<&'a str>,
    /// Optionally validate arbitrary texts in the response html.
    texts: Vec<&'a str>,
    /// Optionally validate the response headers.
    headers: Vec<&'a Header<'a>>,
    /// Optionally validate whether or not the page redirects
    redirect: Option<bool>,
}
impl<'a> Validate<'a> {
    /// Convenience function to bring [`ValidateBuilder`] into scope.
    pub fn builder() -> ValidateBuilder<'a> {
        ValidateBuilder::new()
    }

    /// Create a [`Validate`] object that performs no validation.
    ///
    /// This is useful to load all static assets and return the body of the response.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    ///
    /// let _validate = Validate::none();
    /// ```
    pub fn none() -> Validate<'a> {
        Validate::builder().build()
    }
}

/// Used to build a [`Validate`] object, necessary to invoke the
/// [`validate_and_load_static_assets`] function.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{validate_and_load_static_assets, Validate};
///
/// task!(load_and_validate_page);
///
/// async fn load_and_validate_page(user: &mut GooseUser) -> GooseTaskResult {
///     // Make a GET request.
///     let mut goose = user.get("example/path").await?;
///
///     // Build a [`Validate`] object to confirm the response is valid.
///     let validate = &Validate::builder()
///         // Validate that the page has `Example` in the title.
///         .title("Example")
///         // Validate that the page has `foo` in the returned html body.
///         .text("foo")
///         // Validate that the page also has `<a href="bar">` in the returned
///         // html body.
///         .text(r#"<a href="bar">"#)
///         .build();
///
///     // Perform the actual validation, using `?` to pass up the error if any
///     // validation fails.
///     validate_and_load_static_assets(
///         user,
///         goose,
///         &validate,
///     ).await?;
///
///     Ok(())
/// }
#[derive(Clone, Debug)]
pub struct ValidateBuilder<'a> {
    /// Optionally validate the response status code.
    status: Option<u16>,
    /// Optionally validate the response title.
    title: Option<&'a str>,
    /// Optionally validate arbitrary texts in the response html.
    texts: Vec<&'a str>,
    /// Optionally validate the response headers.
    headers: Vec<&'a Header<'a>>,
    /// Optionally validate whether or not the page redirects
    redirect: Option<bool>,
}
impl<'a> ValidateBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            status: None,
            title: None,
            texts: vec![],
            headers: vec![],
            redirect: None,
        }
    }

    /// Define the HTTP status expected to be returned when loading the page.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::builder()
    ///     .status(200)
    ///     .build();
    /// ```
    pub fn status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    /// Create a [`Validate`] object to validate that response title contains the specified
    /// text.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::builder()
    ///     .title("Home page")
    ///     .build();
    /// ```
    pub fn title(mut self, title: impl Into<&'a str>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Create a [`Validate`] object to validate that the response page contains the specified
    /// text.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::builder()
    ///     .text("example")
    ///     .build();
    /// ```
    ///
    /// It's possible to call this function multiple times to validate that multiple texts
    /// appear on the page. Alternatively you can call [`ValidateBuilder::texts`].
    ///
    /// # Multiple Example
    /// ```rust
    /// use goose_eggs::Validate;
    ///
    /// let _validate = Validate::builder()
    ///     .text("example")
    ///     .text("another")
    ///     .build();
    /// ```
    pub fn text(mut self, text: &'a str) -> Self {
        self.texts.push(text);
        self
    }

    /// Create a [`Validate`] object to validate that the response page contains the specified
    /// texts.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    ///
    /// let _validate = Validate::builder()
    ///     .texts(vec!["example", "another"])
    ///     .build();
    /// ```
    ///
    /// Alternatively you can call [`ValidateBuilder::text`].
    pub fn texts(mut self, texts: Vec<&'a str>) -> Self {
        self.texts = texts;
        self
    }

    /// Create a [`Validate`] object to validate that the response includes the specified
    /// header.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::builder()
    ///     .header(&Header::name("x-cache"))
    ///     .build();
    /// ```
    ///
    /// It's possible to call this function multiple times to validate multiple headers.
    /// Alternatively you can call [`ValidateBuilder::headers`].
    ///
    /// # Multiple Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::builder()
    ///     .header(&Header::name("x-cache"))
    ///     .header(&Header::name("x-generator"))
    ///     .build();
    /// ```
    pub fn header(mut self, header: &'a Header<'a>) -> Self {
        self.headers.push(header);
        self
    }

    /// Create a [`Validate`] object to validate that the response page contains the specified
    /// headers.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Validate, Header};
    ///
    /// let _validate = Validate::builder()
    ///     .headers(vec![&Header::name("x-cache"), &Header::name("x-generator")])
    ///     .build();
    /// ```
    ///
    /// Alternatively you can call [`ValidateBuilder::header`].
    pub fn headers(mut self, headers: Vec<&'a Header<'a>>) -> Self {
        self.headers = headers;
        self
    }

    /// Create a [`Validate`] object to validate whether or not the response page redirected.
    ///
    /// This structure is passed to [`validate_and_load_static_assets`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    ///
    /// // Verify the response redirected.
    /// let _validate = Validate::builder().redirect(true).build();
    ///
    /// // Verify the response did not redirect.
    /// let _validate = Validate::builder().redirect(false).build();
    /// ```
    pub fn redirect(mut self, redirect: impl Into<bool>) -> Self {
        self.redirect = Some(redirect.into());
        self
    }

    /// Build the [`Validate`] object which is then passed to the
    /// [`validate_and_load_static_assets`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    ///
    /// // Use the default search form to search for `example keys`.
    /// let _validate = Validate::builder()
    ///     .text("example text")
    ///     .build();
    /// ```
    pub fn build(self) -> Validate<'a> {
        let Self {
            status,
            title,
            texts,
            headers,
            redirect,
        } = self;
        Validate {
            status,
            title,
            texts,
            headers,
            redirect,
        }
    }
}

/// Used to validate that headers are included in the server response.
///
/// # Example
/// ```rust
/// use goose_eggs::Header;
///
/// fn example() {
///     // Validate that the "x-varnish" header is set.
///     let _header = Header::name("x-varnish");
/// }

#[derive(Clone, Debug)]
pub struct Header<'a> {
    /// The name of the header to validate, required.
    name: &'a str,
    /// The value of the header to validate, optional.
    value: Option<&'a str>,
}
impl<'a> Header<'a> {
    /// Create a new Header validation struct by specifying all fields.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Header;
    ///
    /// let _header = Header::new("foo", Some("bar"));
    /// ```
    pub fn new(name: &'a str, value: Option<&'a str>) -> Header<'a> {
        Header { name, value }
    }

    /// Create a Header object to validate that a named header is set.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Header;
    ///
    /// // Create a Header object to validate that the "foo" header is set in the Response.
    /// let _header = Header::name("foo");
    /// ```
    pub fn name(name: &'a str) -> Header<'a> {
        Header::new(name, None)
    }

    /// Create a Header object to validate that a named header contains a specific value.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Header;
    ///
    /// // Create a Header object to validate that the "foo" header is set and contains "bar"
    /// // in the Response.
    /// let _header = Header::name_value("foo", "bar");
    /// ```
    pub fn name_value(name: &'a str, value: &'a str) -> Header<'a> {
        Header::new(name, Some(value))
    }
}

/// Use a regular expression to get the HTML header from the web page.
///
/// # Example
/// ```rust
/// use goose_eggs::get_html_header;
///
/// // For this example we grab just a subset of a web page, enough to demonstrate. Normally
/// // you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr">
///   <head>
///     <meta charset="utf-8" />
///     <link rel="canonical" href="https://example.com/" />
///     <link rel="shortlink" href="https://example.com/" />
///     <meta name="Generator" content="Drupal 9 (https://www.drupal.org)" />
///     <meta name="MobileOptimized" content="width" />
///     <meta name="HandheldFriendly" content="true" />
///     <meta name="viewport" content="width=device-width, initial-scale=1.0" />
///     <title>Example Website</title>
///   </head>
/// <body>
///   This is the web page body.
/// </body>
/// </html>
/// "#;
///
/// let html_header = get_html_header(html);
/// assert!(!html_header.is_none());
/// ```
pub fn get_html_header(html: &str) -> Option<String> {
    let re = Regex::new(r#"<head(.*?)</head>"#).unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire html header, a subset of the received html.
    re.captures(&line).map(|value| value[0].to_string())
}

/// Use a regular expression to get the web page title.
///
/// # Example
/// ```rust
/// use goose_eggs::{get_html_header, get_title};
///
/// // For this example we grab just a subset of a web page, enough to demonstrate. Normally
/// // you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr">
///   <head>
///     <meta charset="utf-8" />
///     <link rel="canonical" href="https://example.com/" />
///     <link rel="shortlink" href="https://example.com/" />
///     <meta name="Generator" content="Drupal 9 (https://www.drupal.org)" />
///     <meta name="MobileOptimized" content="width" />
///     <meta name="HandheldFriendly" content="true" />
///     <meta name="viewport" content="width=device-width, initial-scale=1.0" />
///     <title>Example Website</title>
///   </head>
/// <body>
///   This is the web page body.
/// </body>
/// </html>
/// "#;
///
/// // Start by extracting the HTML header from the HTML.
/// let html_header = get_html_header(html).map_or_else(|| "".to_string(), |h| h.to_string());
/// // Next extract the title from the HTML header.
/// let title = get_title(&html_header).map_or_else(|| "".to_string(), |t| t.to_string());
/// assert_eq!(title, "Example Website");
/// ```
pub fn get_title(html: &str) -> Option<String> {
    let re = Regex::new(r#"<title>(.*?)</title>"#).unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire title, a subset of the received html.
    re.captures(&line).map(|value| value[1].to_string())
}

/// Returns a [`bool`] indicating whether or not the title (case insensitive) on the
/// webpage contains the provided string.
///
/// While you can invoke this function directly, it's generally preferred to invoke
/// [`validate_and_load_static_assets`] which in turn invokes this function.
///
/// A valid title is found between `<title></title>` tags inside `<head></head>` tags.
/// For example, if the title is as follows:
/// ```html
/// <head>
///   <title>this is the title</title>
/// </head>
/// ```
///
/// Then a call to `valid_title("the title")` will return [`true`], whereas a call to
/// `valid_title("foo")` will return [`false`].
///
/// This function is case insensitive, so in the above example calling
/// `valid_title("The Title")` and `valid_title("THE TITLE")` will both also return
/// [`true`]. The function only tests if the title includes the specified text, the
/// title can also include other text and will still be considered valid.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::valid_title;
///
/// task!(validate_title).set_on_start();
///
/// async fn validate_title(user: &mut GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             match response.text().await {
///                 Ok(html) => {
///                     // Confirm that the HTML header includes the expected title.
///                     let title = "example";
///                     if !valid_title(&html, title) {
///                         return user.set_failure(
///                             &format!("{}: title not found: {}", goose.request.raw.url, title),
///                             &mut goose.request,
///                             Some(headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.raw.url, e),
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
    // Extract the HTML header from the provided html.
    let html_header = get_html_header(html).map_or_else(|| "".to_string(), |h| h);
    // Next extract the title from the HTML header.
    let html_title = get_title(&html_header).map_or_else(|| "".to_string(), |t| t);
    // Finally, confirm that the title contains the expected text.
    html_title
        .to_ascii_lowercase()
        .contains(title.to_ascii_lowercase().as_str())
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
/// async fn validate_text(user: &mut GooseUser) -> GooseTaskResult {
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
///                             &format!("{}: text not found: {}", goose.request.raw.url, text),
///                             &mut goose.request,
///                             Some(headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.raw.url, e),
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
/// async fn validate_header(user: &mut GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             if !header_is_set(headers, &Header::name("server")) {
///                 return user.set_failure(
///                     &format!("{}: header not found: {}", goose.request.raw.url, "server"),
///                     &mut goose.request,
///                     Some(headers),
///                     None,
///                 );
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.raw.url, e),
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
/// async fn validate_header_value(user: &mut GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             if !valid_header_value(headers, &Header::name_value("server", "nginx")) {
///                 return user.set_failure(
///                     &format!("{}: server header value not correct: {}", goose.request.raw.url, "nginx"),
///                     &mut goose.request,
///                     Some(headers),
///                     None,
///                 );
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.raw.url, e),
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
/// async fn load_page_and_static_elements(user: &mut GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///
///     match goose.response {
///         Ok(response) => {
///             // Copy the headers so we have them for logging if there are errors.
///             let headers = &response.headers().clone();
///             match response.text().await {
///                 Ok(html) => {
///                     // Load all static elements on page.
///                     load_static_elements(user, &html).await;
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(headers),
///                         None,
///                     );
///                 }
///             }
///         }
///         Err(e) => {
///             return user.set_failure(
///                 &format!("{}: no response from server: {}", goose.request.raw.url, e),
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
pub async fn load_static_elements(user: &mut GooseUser, html: &str) {
    // Determine the base_url that was used to load this path, used to extract absolute URLs.
    let base_url = user.base_url.to_string();

    // Use a case-insensitive regular expression to find all src=<foo> in the html, where
    // <foo> is the URL to local image and js assets.
    // @TODO: parse HTML5 srcset= also
    let image = Regex::new(format!(r#"(?i)src="(({}|/).*?)""#, base_url).as_str()).unwrap();
    let mut urls = Vec::new();
    for url in image.captures_iter(html) {
        urls.push(url[1].to_string());
    }

    // Use a case-insensitive regular expression to find all href=<foo> in the html, where
    // <foo> is the URL to local css assets.
    let css = Regex::new(format!(r#"(?i)href="(({}|/).*?\.css.*?)""#, base_url).as_str()).unwrap();
    for url in css.captures_iter(html) {
        urls.push(url[1].to_string());
    }

    // Load all the static assets found on the page.
    for asset in &urls {
        let _ = user.get_named(asset, "static asset").await;
    }
}

/// Validate the HTML response, extract and load all static elements on the page, and
/// return the HTML body.
///
/// What is validated is defined with the [`Validate`] structure.
///
/// If the page doesn't load, an empty [`String`] will be returned. If the page does load
/// but validation fails, an Error is returned. If the page loads and there are no
/// errors the body is returned as a [`String`].
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{validate_and_load_static_assets, Validate};
///
/// task!(load_page).set_on_start();
///
/// async fn load_page(user: &mut GooseUser) -> GooseTaskResult {
///     let mut goose = user.get("/").await?;
///     validate_and_load_static_assets(
///         user,
///         goose,
///         // Validate title and other arbitrary text on the response html.
///         &Validate::builder()
///             .title("my page")
///             .texts(vec!["foo", r#"<a href="bar">"#])
///             .build(),
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub async fn validate_and_load_static_assets<'a>(
    user: &mut GooseUser,
    mut goose: GooseResponse,
    validate: &'a Validate<'a>,
) -> Result<String, GooseTaskError> {
    let empty = "".to_string();
    match goose.response {
        Ok(response) => {
            // Validate whether or not the request redirected.
            if let Some(redirect) = validate.redirect {
                if goose.request.redirected != redirect {
                    // Get as much as we can from the response for useful debug logging.
                    let headers = &response.headers().clone();
                    let html = response.text().await.unwrap_or_else(|_| "".to_string());
                    let error = if redirect {
                        format!("{}: did not redirect", goose.request.raw.url)
                    // Unexpected redirect happened.
                    } else {
                        format!("{}: redirected unexpectedly", goose.request.raw.url)
                    };
                    user.set_failure(&error, &mut goose.request, Some(headers), Some(&html))?;
                    // Exit as soon as validation fails, to avoid cascades of
                    // errors whe na page fails to load.
                    return Ok(html);
                }
            }

            // Validate status code if defined.
            if let Some(status) = validate.status {
                if response.status() != status {
                    // Get as much as we can from the response for useful debug logging.
                    let headers = &response.headers().clone();
                    let response_status = response.status();
                    let html = response.text().await.unwrap_or_else(|_| "".to_string());
                    user.set_failure(
                        &format!(
                            "{}: response status != {}]: {}",
                            goose.request.raw.url, status, response_status
                        ),
                        &mut goose.request,
                        Some(headers),
                        Some(&html),
                    )?;
                    // Exit as soon as validation fails, to avoid cascades of
                    // errors whe na page fails to load.
                    return Ok(html);
                }
            }

            // Validate headers if defined.
            let headers = &response.headers().clone();
            for header in &validate.headers {
                if !header_is_set(headers, header) {
                    // Get as much as we can from the response for useful debug logging.
                    let html = response.text().await.unwrap_or_else(|_| "".to_string());
                    user.set_failure(
                        &format!(
                            "{}: header not included in response: {:?}",
                            goose.request.raw.url, header
                        ),
                        &mut goose.request,
                        Some(headers),
                        Some(&html),
                    )?;
                    // Exit as soon as validation fails, to avoid cascades of
                    // errors when a page fails to load.
                    return Ok(html);
                }
                if let Some(h) = header.value {
                    if !valid_header_value(headers, header) {
                        // Get as much as we can from the response for useful debug logging.
                        let html = response.text().await.unwrap_or_else(|_| "".to_string());
                        user.set_failure(
                            &format!(
                                "{}: header does not contain expected value: {:?}",
                                goose.request.raw.url, h
                            ),
                            &mut goose.request,
                            Some(headers),
                            Some(&html),
                        )?;
                        // Exit as soon as validation fails, to avoid cascades of
                        // errors when a page fails to load.
                        return Ok(html);
                    }
                }
            }

            // Extract the response body to validate and load static elements.
            match response.text().await {
                Ok(html) => {
                    // Validate title if defined.
                    if let Some(title) = validate.title {
                        if !valid_title(&html, title) {
                            user.set_failure(
                                &format!("{}: title not found: {}", goose.request.raw.url, title),
                                &mut goose.request,
                                Some(headers),
                                Some(&html),
                            )?;
                            // Exit as soon as validation fails, to avoid cascades of
                            // errors when a page fails to load.
                            return Ok(html);
                        }
                    }
                    // Validate texts in body if defined.
                    for text in &validate.texts {
                        if !valid_text(&html, text) {
                            user.set_failure(
                                &format!(
                                    "{}: text not found on page: {}",
                                    goose.request.raw.url, text
                                ),
                                &mut goose.request,
                                Some(headers),
                                Some(&html),
                            )?;
                            // Exit as soon as validation fails, to avoid cascades of
                            // errors when a page fails to load.
                            return Ok(html);
                        }
                    }
                    load_static_elements(user, &html).await;
                    Ok(html)
                }
                Err(e) => {
                    user.set_failure(
                        &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
                        &mut goose.request,
                        Some(headers),
                        None,
                    )?;
                    Ok(empty)
                }
            }
        }
        Err(e) => {
            user.set_failure(
                &format!("{}: no response from server: {}", goose.request.raw.url, e),
                &mut goose.request,
                None,
                None,
            )?;
            Ok(empty)
        }
    }
}
