//! Goose Eggs are helpful in writing [`Goose`](https://docs.rs/goose) load tests.
use goose::goose::GooseResponse;
use goose::prelude::*;
use log::info;
use regex::Regex;
use reqwest::header::HeaderMap;

pub mod drupal;

/// Define one or more items to be validated in a web page response.
///
/// This structure is passed to [`validate_and_load_static_assets`].
///
/// # Example
/// ```rust
/// use goose_eggs::Validate;
///
/// fn examples() {
///     // Manually build a Validate strucuture that validates the page title and
///     // some arbitrary texts in the response html.
///     let _validate = Validate::new(
///         None, Some("my page"), vec!["foo", r#"<a href="bar">"#], vec![]
///     );
///
///     // Use `title_texts()` helper to perform the same validation.
///     let _validate = Validate::title_texts("my page", vec!["foo", r#"<a href="bar">"#]);
///
///     // Use `title_text()` helper to perform similar validation, validating only
///     // one text on the page.
///     let _validate = Validate::title_text("my page", r#"<a href="foo">"#);
/// }
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
}
impl<'a> Validate<'a> {
    /// Create a new Validate struct, specifying `status`, `title`, `texts` and `headers`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::new(
    ///     // Validate the response status code.
    ///     Some(404),
    ///     // Validate the response page title.
    ///     Some("Page Not Found"),
    ///     // Validate arbitrary text on the response page.
    ///     vec!["Oops, something went wrong!"],
    ///     // Validate that the response was sent via https.
    ///     vec![&Header::name_value("scheme", "https")],
    /// );
    /// ```
    pub fn new(
        status: Option<u16>,
        title: Option<&'a str>,
        texts: Vec<&'a str>,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate {
            status,
            title,
            texts,
            headers,
        }
    }

    /// Create a Validate object to validate the response status code.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status(200);
    /// ```
    pub fn status(status: u16) -> Validate<'a> {
        Validate::new(Some(status), None, vec![], vec![])
    }

    /// Create a Validate object to validate the response title.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title("Home page");
    /// ```
    pub fn title(title: &'a str) -> Validate<'a> {
        Validate::new(None, Some(title), vec![], vec![])
    }

    /// Create a Validate object to validate the response status code and title.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title(200, "Home page");
    /// ```
    pub fn status_title(status: u16, title: &'a str) -> Validate<'a> {
        Validate::new(Some(status), Some(title), vec![], vec![])
    }

    /// Create a Validate object to validate specific text is on the response page.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::text("This text should be on the page.");
    /// ```
    pub fn text(text: &'a str) -> Validate<'a> {
        Validate::new(None, None, vec![text], vec![])
    }

    /// Create a Validate object to validate the response has the correct status code and
    /// contains specific text.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_text(200, "This text should be on the page.");
    /// ```
    pub fn status_text(status: u16, text: &'a str) -> Validate<'a> {
        Validate::new(Some(status), None, vec![text], vec![])
    }

    /// Create a Validate object to validate the response has the correct title and also
    /// contains specific text.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_text("Example", "This text should be on the page.");
    /// ```
    pub fn title_text(title: &'a str, text: &'a str) -> Validate<'a> {
        Validate::new(None, Some(title), vec![text], vec![])
    }

    /// Create a Validate object to validate the response code, that the page has the correct
    /// title and also contains specific text.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_text(200, "Example", "This text should be on the page.");
    /// ```
    pub fn status_title_text(status: u16, title: &'a str, text: &'a str) -> Validate<'a> {
        Validate::new(Some(status), Some(title), vec![text], vec![])
    }

    /// Create a Validate object to validate that the response page contains multiple specific
    /// texts.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::texts(vec!["This text should be on the page.", "And also this", r#"<span>and this</a>"#]);
    /// ```
    pub fn texts(texts: Vec<&'a str>) -> Validate<'a> {
        Validate::new(None, None, texts, vec![])
    }

    /// Create a Validate object to validate response status code and that the page contains
    /// multiple specific texts.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_texts(200, vec!["This text should be on the page.", "And also this", r#"<span>and this</a>"#]);
    /// ```
    pub fn status_texts(status: u16, texts: Vec<&'a str>) -> Validate<'a> {
        Validate::new(Some(status), None, texts, vec![])
    }

    /// Create a Validate object to validate the response title and that the page contains
    /// multiple specific texts.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_texts("Example", vec!["This text should be on the page.", "And also this", r#"<span>and this</a>"#]);
    /// ```
    pub fn title_texts(title: &'a str, texts: Vec<&'a str>) -> Validate<'a> {
        Validate::new(None, Some(title), texts, vec![])
    }

    /// Create a Validate object to validate the response status code, the page title and that the
    /// page contains multiple specific texts.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_texts(200, "Example", vec!["This text should be on the page.", "And also this", r#"<span>and this</a>"#]);
    /// ```
    pub fn status_title_texts(status: u16, title: &'a str, texts: Vec<&'a str>) -> Validate<'a> {
        Validate::new(Some(status), Some(title), texts, vec![])
    }

    /// Create a Validate object to validate the response included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::header(&Header::name("x-cache"));
    /// ```
    pub fn header(header: &'a Header<'a>) -> Validate<'a> {
        Validate::new(None, None, vec![], vec![header])
    }

    /// Create a Validate object to validate the response status code and that it included a
    /// specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_header(200, &Header::name("x-cache"));
    /// ```
    pub fn status_header(status: u16, header: &'a Header<'a>) -> Validate<'a> {
        Validate::new(Some(status), None, vec![], vec![header])
    }

    /// Create a Validate object to validate the response title and that it included a
    /// specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_header("example", &Header::name("x-cache"));
    /// ```
    pub fn title_header(title: &'a str, header: &'a Header<'a>) -> Validate<'a> {
        Validate::new(None, Some(title), vec![], vec![header])
    }

    /// Create a Validate object to validate the response status code, title and that it
    /// included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_header(200, "example", &Header::name("x-cache"));
    /// ```
    pub fn status_title_header(
        status: u16,
        title: &'a str,
        header: &'a Header<'a>,
    ) -> Validate<'a> {
        Validate::new(Some(status), Some(title), vec![], vec![header])
    }

    /// Create a Validate object to validate the response html contains specific text and that it
    /// included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::text_header("The page has this text", &Header::name("x-cache"));
    /// ```
    pub fn text_header(text: &'a str, header: &'a Header<'a>) -> Validate<'a> {
        Validate::new(None, None, vec![text], vec![header])
    }

    /// Create a Validate object to validate the response status code, that the resposne html
    /// contains specific text and that it included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_text_header(200, "The page has this text", &Header::name("x-cache"));
    /// ```
    pub fn status_text_header(status: u16, text: &'a str, header: &'a Header<'a>) -> Validate<'a> {
        Validate::new(Some(status), None, vec![text], vec![header])
    }

    /// Create a Validate object to validate the response html title, that it contains
    /// specific text and that it included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_text_header("Example", "The page has this text", &Header::name("x-cache"));
    /// ```
    pub fn title_text_header(
        title: &'a str,
        text: &'a str,
        header: &'a Header<'a>,
    ) -> Validate<'a> {
        Validate::new(None, Some(title), vec![text], vec![header])
    }

    /// Create a Validate object to validate the response status code, the  html title, that it
    /// contains specific text and that it included a specific header.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_text_header(200, "Example", "The page has this text", &Header::name("x-cache"));
    /// ```
    pub fn status_title_text_header(
        status: u16,
        title: &'a str,
        text: &'a str,
        header: &'a Header<'a>,
    ) -> Validate<'a> {
        Validate::new(Some(status), Some(title), vec![text], vec![header])
    }

    /// Create a Validate object to validate that the response included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::headers(vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn headers(headers: Vec<&'a Header<'a>>) -> Validate<'a> {
        Validate::new(None, None, vec![], headers)
    }

    /// Create a Validate object to validate the response status code and that it included multiple
    /// specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_headers(200, vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn status_headers(status: u16, headers: Vec<&'a Header<'a>>) -> Validate<'a> {
        Validate::new(Some(status), None, vec![], headers)
    }

    /// Create a Validate object to validate the response html title and that it included multiple
    /// specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_headers("Example", vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn title_headers(title: &'a str, headers: Vec<&'a Header<'a>>) -> Validate<'a> {
        Validate::new(None, Some(title), vec![], headers)
    }

    /// Create a Validate object to validate the response status code, the html title and that it
    /// included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_headers(200, "Example", vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn status_title_headers(
        status: u16,
        title: &'a str,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(Some(status), Some(title), vec![], headers)
    }

    /// Create a Validate object to validate the response html contained specific text and that
    /// the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::text_headers("This text is on the page", vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn text_headers(text: &'a str, headers: Vec<&'a Header<'a>>) -> Validate<'a> {
        Validate::new(None, None, vec![text], headers)
    }

    /// Create a Validate object to validate the response status code, that the html contained
    /// specific text and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_text_headers(200, "This text is on the page", vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn status_text_headers(
        status: u16,
        text: &'a str,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(Some(status), None, vec![text], headers)
    }

    /// Create a Validate object to validate the response html title, that the html contained
    /// specific text and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_text_headers("Example", "This text is on the page", vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn title_text_headers(
        title: &'a str,
        text: &'a str,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(None, Some(title), vec![text], headers)
    }

    /// Create a Validate object to validate the response html includes multiple specific texts
    /// and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::texts_headers(vec!["This text is on the page", r#"<a href="/foo">and so is this</a>"#], vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn texts_headers(texts: Vec<&'a str>, headers: Vec<&'a Header<'a>>) -> Validate<'a> {
        Validate::new(None, None, texts, headers)
    }

    /// Create a Validate object to validate the response status code, that html includes
    /// multiple specific texts and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_texts_headers(200, vec!["This text is on the page", r#"<a href="/foo">and so is this</a>"#], vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn status_texts_headers(
        status: u16,
        texts: Vec<&'a str>,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(Some(status), None, texts, headers)
    }

    /// Create a Validate object to validate the response html title, that html includes
    /// multiple specific texts and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::title_texts_headers("Example", vec!["This text is on the page", r#"<a href="/foo">and so is this</a>"#], vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn title_texts_headers(
        title: &'a str,
        texts: Vec<&'a str>,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(None, Some(title), texts, headers)
    }

    /// Create a Validate object to validate the response code, that html includes
    /// multiple specific texts and that the response also included multiple specific headers.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{Header, Validate};
    ///
    /// let _validate = Validate::status_title_texts_headers(200, "Example", vec!["This text is on the page", r#"<a href="/foo">and so is this</a>"#], vec![&Header::name("x-cache"), &Header::name("x-generator")]);
    /// ```
    pub fn status_title_texts_headers(
        status: u16,
        title: &'a str,
        texts: Vec<&'a str>,
        headers: Vec<&'a Header<'a>>,
    ) -> Validate<'a> {
        Validate::new(Some(status), Some(title), texts, headers)
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
    let re = Regex::new(r#"<head>(.*?)</head>"#).unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire html header, a subset of the received html.
    re.captures(&line).map(|value| value[0].to_string())
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
/// use goose_eggs::{get_html_header, valid_title};
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
///                     // First confirm that the provided HTML has a header.
///                     let header = get_html_header(&html).map_or_else(|| "".to_string(), |h| h.to_string());
///                     if header.is_empty() {
///                         return user.set_failure(
///                             &format!("{}: no html header found", goose.request.raw.url),
///                             &mut goose.request,
///                             Some(&headers),
///                             Some(&html),
///                         );
///                     }
///                     // Finally confirm that the HTML header includes the expected title.
///                     let title = "example";
///                     if !valid_title(&header, title) {
///                         return user.set_failure(
///                             &format!("{}: title not found: {}", goose.request.raw.url, title),
///                             &mut goose.request,
///                             Some(&headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(&headers),
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
///                             &format!("{}: text not found: {}", goose.request.raw.url, text),
///                             &mut goose.request,
///                             Some(&headers),
///                             Some(&html),
///                         );
///                     }
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(&headers),
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
/// async fn validate_header(user: &GooseUser) -> GooseTaskResult {
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
///                     Some(&headers),
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
/// async fn validate_header_value(user: &GooseUser) -> GooseTaskResult {
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
///                     Some(&headers),
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
///                     load_static_elements(user, &html).await;
///                 }
///                 Err(e) => {
///                     return user.set_failure(
///                         &format!("{}: failed to parse page: {}", goose.request.raw.url, e),
///                         &mut goose.request,
///                         Some(&headers),
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

/// Validate the HTML response, extract and load all static elements on the page, and
/// return the HTML body.
///
/// What is validated is defined with the [`Validate`] structure.
///
/// If the page doesn't load, an empty String will be returned. If the page does load
/// but validation fails, an Error is returned. If the page loads and there are no
/// errors the body is returned as a String.
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
///         // Validate title and other arbitrary text on the response html.
///         &Validate::title_texts("my page", vec!["foo", r#"<a href="bar">"#]),
///     ).await?;
///
///     Ok(())
/// }
/// ```
pub async fn validate_and_load_static_assets<'a>(
    user: &GooseUser,
    mut goose: GooseResponse,
    validate: &'a Validate<'a>,
) -> Result<String, GooseTaskError> {
    let empty = "".to_string();
    match goose.response {
        Ok(response) => {
            // Validate status code if defined.
            let response_status = response.status();
            if let Some(status) = validate.status {
                if response_status != status {
                    // Get as much as we can from the response for useful debug logging.
                    let headers = &response.headers().clone();
                    let html = response.text().await.unwrap_or_else(|_| "".to_string());
                    user.set_failure(
                        &format!(
                            "{}: response status != {}]: {}",
                            goose.request.raw.url, status, response_status
                        ),
                        &mut goose.request,
                        Some(&headers),
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
                        Some(&headers),
                        Some(&html),
                    )?;
                    // Exit as soon as validation fails, to avoid cascades of
                    // errors whe na page fails to load.
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
                            Some(&headers),
                            Some(&html),
                        )?;
                        // Exit as soon as validation fails, to avoid cascades of
                        // errors whe na page fails to load.
                        return Ok(html);
                    }
                }
            }

            // Extract the response body to validate and load static elements.
            match response.text().await {
                Ok(html) => {
                    // Validate title if defined.
                    if let Some(title) = validate.title {
                        if !valid_title(&html, &title) {
                            user.set_failure(
                                &format!("{}: title not found: {}", goose.request.raw.url, title),
                                &mut goose.request,
                                Some(&headers),
                                Some(&html),
                            )?;
                            // Exit as soon as validation fails, to avoid cascades of
                            // errors whe na page fails to load.
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
                                Some(&headers),
                                Some(&html),
                            )?;
                            // Exit as soon as validation fails, to avoid cascades of
                            // errors whe na page fails to load.
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
                        Some(&headers),
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
