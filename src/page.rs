//! Loading a WordPress page the way a browser does: the page, the static assets
//! its markup asks for, and the background requests its own scripts send when
//! it opens. Shared by the `wordpress` and `woocommerce` helpers, and private.
//!
//! What a browser requests for each action on the demo sites is recorded under
//! `tests/fixtures/browser/`; the rules here follow those recordings.

use crate::wordpress::{attribute, read_response, request_ok};
use goose::goose::GooseResponse;
use goose::prelude::*;
use regex::Regex;

/// A page as a browser left it once it opened: its markup, and what its
/// background requests answered that a helper may need next.
pub(crate) struct LoadedPage {
    /// The page's HTML.
    pub html: String,
    /// The Store API `Nonce` header, when the page's scripts read the Store API
    /// cart (as WooCommerce's product buttons do); the cart's next write sends it.
    pub store_api_nonce: Option<String>,
}

// The markup a browser reads: comments, <noscript> fallbacks and <template>s
// removed, as a browser with scripts on loads none of them, and every script's
// body emptied (its tag stays), so markup in a script (WordPress's Underscore
// templates, `<script type="text/html">`) is not read as the page's.
fn live_markup(html: &str) -> String {
    // One pass, left to right, as a browser's parser reads them: a "<!--" in a
    // script is script, and a "<script" in a comment is comment.
    let hidden = Regex::new(
        r"(?is)<!--.*?-->|(<script\b[^>]*>).*?</script\s*>|<noscript\b.*?</noscript\s*>|<template\b.*?</template\s*>",
    )
    .unwrap();
    hidden
        .replace_all(html, |c: &regex::Captures| match c.get(1) {
            Some(open) => format!("{}</script>", open.as_str()),
            None => " ".to_string(),
        })
        .into_owned()
}

/// The static assets a browser loads for `html`, in document order, each once:
/// every script with a `src` (classic or module), every stylesheet, every
/// `preload`, `modulepreload` and `prefetch` link, and every image that is not
/// lazy loaded. Fonts and images that only CSS names, scripts that other scripts
/// load, `srcset` variants and lazy loaded images are not included: which of
/// them a browser fetches depends on its CSS engine, its scripts and its
/// viewport.
pub(crate) fn page_assets(html: &str) -> Vec<(String, &'static str)> {
    let html = live_markup(html);
    let tag = Regex::new(r"(?is)<(script|link|img)\b[^>]*>").unwrap();
    let mut assets: Vec<(String, &'static str)> = Vec::new();
    for m in tag.find_iter(&html) {
        let tag = m.as_str();
        let name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let asset = match name.as_str() {
            "script" => {
                let kind = attribute(tag, "type")
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let runs = kind.is_empty()
                    || kind == "module"
                    || kind.contains("javascript")
                    || kind.contains("ecmascript");
                match attribute(tag, "src") {
                    Some(src) if runs && !src.is_empty() => Some((src, "js")),
                    _ => None,
                }
            }
            "link" => {
                let rel = attribute(tag, "rel")
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                let rels: Vec<&str> = rel.split_whitespace().collect();
                let href = attribute(tag, "href").unwrap_or_default();
                if href.is_empty() {
                    None
                } else if rels.contains(&"stylesheet") {
                    Some((href, "css"))
                } else if rels
                    .iter()
                    .any(|r| matches!(*r, "preload" | "modulepreload" | "prefetch"))
                {
                    let kind = match attribute(tag, "as").as_deref() {
                        Some("style") => "css",
                        Some("image") => "img",
                        Some("font") => "font",
                        _ => "js",
                    };
                    Some((href, kind))
                } else {
                    None
                }
            }
            _ => {
                let lazy =
                    attribute(tag, "loading").is_some_and(|l| l.eq_ignore_ascii_case("lazy"));
                match attribute(tag, "src") {
                    Some(src) if !lazy && !src.is_empty() && !src.starts_with("data:") => {
                        Some((src, "img"))
                    }
                    _ => None,
                }
            }
        };
        if let Some((url, kind)) = asset {
            if !assets.iter().any(|(u, _)| *u == url) {
                assets.push((url, kind));
            }
        }
    }
    assets
}

// Whether `url` is on the load test's host: a path, or an absolute or protocol
// relative URL naming the same host and port.
pub(crate) fn same_site(user: &GooseUser, url: &str) -> bool {
    let absolute = if let Some(rest) = url.strip_prefix("//") {
        format!("{}://{}", user.base_url.scheme(), rest)
    } else {
        url.to_string()
    };
    if !Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")
        .unwrap()
        .is_match(&absolute)
    {
        return true;
    }
    match user.base_url.join(&absolute) {
        Ok(parsed) => {
            matches!(parsed.scheme(), "http" | "https")
                && parsed.host_str() == user.base_url.host_str()
                && parsed.port_or_known_default() == user.base_url.port_or_known_default()
        }
        Err(_) => false,
    }
}

/// Load every static asset of `html` that is on the site, as [`page_assets`]
/// lists them, named `static asset: <kind>` as the `goose-eggs` asset loader
/// names them. `page_url` resolves relative URLs.
pub(crate) async fn load_assets(user: &mut GooseUser, page_url: &str, html: &str) {
    for (url, kind) in page_assets(html) {
        if !same_site(user, &url) {
            continue;
        }
        let url = resolve(user, page_url, &url);
        let _ = user
            .get_named(&url, &format!("static asset: {}", kind))
            .await;
    }
}

// `url` made absolute against the page it was found on.
pub(crate) fn resolve(user: &GooseUser, page_url: &str, url: &str) -> String {
    let page = user
        .base_url
        .join(page_url)
        .unwrap_or_else(|_| user.base_url.clone());
    match page.join(url) {
        Ok(url) => url.to_string(),
        Err(_) => url.to_string(),
    }
}

/// The site's REST API root, as WordPress links it from every page's head
/// (`<link rel="https://api.w.org/" href="...">`); `wp-json/` when it is not.
pub(crate) fn rest_root(html: &str) -> String {
    let link = Regex::new(r"(?is)<link\b[^>]*>").unwrap();
    let root = link
        .find_iter(html)
        .map(|m| m.as_str())
        .find(|tag| attribute(tag, "rel").as_deref() == Some("https://api.w.org/"))
        .and_then(|tag| attribute(tag, "href"))
        .map(|href| crate::wordpress::site_path(&href))
        .unwrap_or_else(|| "/wp-json/".to_string());
    root
}

/// The background requests a page's own scripts send when it opens, in the
/// order a browser sends them, after its assets. Three kinds are known:
///
/// - WordPress's preferences, on admin pages whose user has none saved yet
///   (`var serverData = ""` before `__unstableCreatePersistenceLayer`): the
///   persistence layer reads them, `wp/v2/users/me?context=edit`.
/// - WordPress's script compression test, on admin pages that print it
///   (`testCompression.check()`): `admin-ajax.php?action=wp-compression-test`,
///   `test=1`, then `test=2` or `test=no` as the answers lead, as its script does.
/// - WooCommerce's product buttons (`data-wp-interactive="woocommerce/product-button"`,
///   on shop and category archives and under a product's related products):
///   their cart store reads the Store API cart once.
pub(crate) async fn background_requests(
    user: &mut GooseUser,
    html: &str,
) -> Result<Option<String>, Box<TransactionError>> {
    preferences(user, html).await?;
    compression_test(user, html).await?;
    if html.contains(r#"data-wp-interactive="woocommerce/product-button""#) {
        let path = format!("{}wc/store/v1/cart", rest_root(html));
        let name = path.trim_start_matches('/').to_string();
        let goose = crate::wordpress::get_named(user, &path, &name).await?;
        request_ok(&goose)?;
        let (headers, _) = read_response(goose).await;
        return Ok(crate::wordpress::header_value(&headers, "Nonce"));
    }
    Ok(None)
}

// The REST root a page's api-fetch sends to, from its setup
// (`createRootURLMiddleware(...)`), or the root the page links.
pub(crate) fn api_fetch_root(html: &str) -> String {
    Regex::new(r#"createRootURLMiddleware\(\s*["']([^"']+)["']"#)
        .unwrap()
        .captures(html)
        .map(|c| crate::wordpress::site_path(&c[1].replace("\\/", "/")))
        .unwrap_or_else(|| rest_root(html))
}

// The preferences read WordPress's persistence layer makes on an admin page
// when the user has no preferences saved yet.
async fn preferences(user: &mut GooseUser, html: &str) -> Result<(), Box<TransactionError>> {
    let unsaved = Regex::new(r#"var serverData = (""|null);"#)
        .unwrap()
        .is_match(html)
        && html.contains("__unstableCreatePersistenceLayer( serverData");
    let nonce = match crate::wordpress::get_rest_nonce(html) {
        Some(nonce) if unsaved => nonce,
        _ => return Ok(()),
    };
    let path = format!(
        "{}wp/v2/users/me?context=edit&_locale=user",
        api_fetch_root(html)
    );
    let goose = crate::wordpress::send_json(
        user,
        GooseMethod::Get,
        &path,
        "wp-json/wp/v2/users/me",
        &[
            ("X-WP-Nonce", nonce.as_str()),
            ("Accept", "application/json, */*;q=0.1"),
        ],
        None,
    )
    .await?;
    request_ok(&goose)
}

// WordPress's compression test, as the inline script on admin pages runs it.
async fn compression_test(user: &mut GooseUser, html: &str) -> Result<(), Box<TransactionError>> {
    if !html.contains("testCompression.check()") {
        return Ok(());
    }
    let nonce = match Regex::new(r#"var compressionNonce\s*=\s*["']([^"']+)["']"#)
        .unwrap()
        .captures(html)
    {
        Some(c) => c[1].to_string(),
        None => return Ok(()),
    };
    let ajax = Regex::new(r#"var ajaxurl\s*=\s*["']([^"']+)["']"#)
        .unwrap()
        .captures(html)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "/wp-admin/admin-ajax.php".to_string());
    let mut test = "1";
    loop {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let path = format!(
            "{}?action=wp-compression-test&test={}&_ajax_nonce={}&{}",
            ajax, test, nonce, millis
        );
        let goose = crate::wordpress::get_named(
            user,
            &path,
            "wp-admin/admin-ajax.php?action=wp-compression-test",
        )
        .await?;
        let (headers, body) = read_response(goose).await;
        // The script's own rules: a compressed answer to test 1 ends the test
        // with "no"; test 2 ends it with "yes" when the answer is the test text.
        test = match test {
            "1" => {
                let compressed = crate::wordpress::header_value(&headers, "Content-Encoding")
                    .is_some_and(|e| {
                        let e = e.to_ascii_lowercase();
                        e.contains("deflate") || e.contains("gzip")
                    });
                if compressed {
                    "no"
                } else {
                    "2"
                }
            }
            "2" if body.starts_with("\"wpCompressionTest") => "yes",
            "2" => "no",
            _ => return Ok(()),
        };
    }
}

/// Validate a page, load its static assets and send its background requests,
/// failing first if the request failed.
pub(crate) async fn open(
    user: &mut GooseUser,
    goose: GooseResponse,
    validate: Option<&crate::Validate<'_>>,
) -> Result<LoadedPage, Box<TransactionError>> {
    request_ok(&goose)?;
    let page_url = goose.request.final_url.to_string();
    let none = crate::Validate::none();
    let html = crate::validate_page(user, goose, validate.unwrap_or(&none)).await?;
    load_assets(user, &page_url, &html).await;
    let store_api_nonce = background_requests(user, &html).await?;
    Ok(LoadedPage {
        html,
        store_api_nonce,
    })
}

/// Decode `%XX` escapes, as JavaScript's `decodeURIComponent` does; an invalid
/// escape is kept as is.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The REST responses a page preloads for its scripts, by route, as WordPress
/// writes them into `wp.apiFetch.createPreloadingMiddleware(...)`: plain JSON in
/// admin pages, `JSON.parse(decodeURIComponent('...'))` in WooCommerce's
/// block cart and checkout.
pub(crate) fn preloaded(html: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut found = serde_json::Map::new();
    let encoded = Regex::new(
        r"createPreloadingMiddleware\(\s*JSON\.parse\(\s*decodeURIComponent\(\s*'([^']*)'",
    )
    .unwrap();
    for c in encoded.captures_iter(html) {
        if let Ok(serde_json::Value::Object(map)) =
            serde_json::from_str::<serde_json::Value>(&percent_decode(&c[1]))
        {
            found.extend(map);
        }
    }
    let start = Regex::new(r"createPreloadingMiddleware\(\s*\{").unwrap();
    for m in start.find_iter(html) {
        let from = m.end() - 1;
        let mut stream =
            serde_json::Deserializer::from_str(&html[from..]).into_iter::<serde_json::Value>();
        if let Some(Ok(serde_json::Value::Object(map))) = stream.next() {
            found.extend(map);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_assets_cases() {
        let html = concat!(
            "<script src='/a.js?ver=1'></script>",
            r#"<script type="module" src="/m.js"></script>"#,
            r#"<script type="text/template" src="/t.js"></script>"#,
            r#"<script type="speculationrules">{}</script>"#,
            "<link rel='stylesheet' id='x' href='/s.css?ver=2' media='all' />",
            "<link href='/p.js' rel='prefetch' />",
            r#"<link rel="modulepreload" href="/mp.js">"#,
            r#"<link rel="preload" as="font" href="/f.woff2">"#,
            r#"<link rel="alternate" href="/feed/">"#,
            r#"<img src="/eager.png"><img loading="lazy" src="/lazy.png">"#,
            r#"<img src="data:image/gif;base64,R0lGOD">"#,
            "<!-- <script src='/commented.js'></script> -->",
            "<noscript><img src='/noscript.png'></noscript>",
            "<script type='text/html' id='tmpl-x'><img src=\"{{ data.url }}\"></script>",
            "<script src='/a.js?ver=1'></script>",
            "<script src='/x.js?a=1&#038;b=2'></script>",
        );
        let expected = [
            ("/a.js?ver=1", "js"),
            ("/m.js", "js"),
            ("/s.css?ver=2", "css"),
            ("/p.js", "js"),
            ("/mp.js", "js"),
            ("/f.woff2", "font"),
            ("/eager.png", "img"),
            ("/x.js?a=1&b=2", "js"),
        ];
        let got = page_assets(html);
        let got: Vec<(&str, &str)> = got.iter().map(|(u, k)| (u.as_str(), *k)).collect();
        assert_eq!(got, expected, "page_assets case every kind");
    }

    #[test]
    fn percent_decode_cases() {
        // (case, encoded, expected)
        let cases = [
            ("plain", "abc", "abc"),
            ("escapes", "%7B%22a%22%3A1%7D", r#"{"a":1}"#),
            ("utf-8", "caf%C3%A9", "café"),
            ("invalid escape kept", "100%zz", "100%zz"),
            ("escape at the end", "a%2", "a%2"),
        ];
        for (case, encoded, expected) in cases {
            assert_eq!(
                percent_decode(encoded),
                expected,
                "percent_decode case {}",
                case
            );
        }
    }

    #[test]
    fn preloaded_cases() {
        let html = concat!(
            "wp.apiFetch.use( wp.apiFetch.createPreloadingMiddleware( ",
            r#"{"/wp/v2/posts/1?context=edit":{"body":{"id":1}}} ) );"#,
            "wp.apiFetch.createPreloadingMiddleware( JSON.parse( decodeURIComponent( ",
            "'%7B%22%5C%2Fwc%5C%2Fstore%5C%2Fv1%5C%2Fcart%22%3A%7B%22body%22%3A%7B%22items%22%3A%5B%5D%7D%7D%7D' ) ) )",
        );
        let found = preloaded(html);
        assert_eq!(
            found["/wp/v2/posts/1?context=edit"]["body"]["id"], 1,
            "preloaded case plain json"
        );
        assert!(
            found["/wc/store/v1/cart"]["body"]["items"].is_array(),
            "preloaded case encoded json"
        );
        assert!(preloaded("<html></html>").is_empty(), "preloaded case none");
    }

    #[test]
    fn same_site_cases() {
        let configuration =
            goose::config::GooseConfiguration::parse_args_default(&[] as &[&str]).unwrap();
        let base =
            goose::goose::get_base_url(Some("http://example.com:8080/".to_string()), None, None)
                .unwrap();
        let user = GooseUser::new(0, "".into(), base, &configuration, 0, None).unwrap();
        // (case, url, expected)
        let cases = [
            ("path", "/a/", true),
            ("relative", "a/", true),
            ("same host and port", "http://example.com:8080/a/", true),
            ("protocol relative", "//example.com:8080/a/", true),
            ("another port", "http://example.com/a/", false),
            ("another host", "http://other.example:8080/a/", false),
            ("not http", "mailto:a@example.com", false),
        ];
        for (case, url, expected) in cases {
            assert_eq!(same_site(&user, url), expected, "same_site case {}", case);
        }
    }

    #[test]
    fn rest_root_cases() {
        assert_eq!(
            rest_root(
                r#"<link rel="https://api.w.org/" href="http://localhost:18082/blog/wp-json/" />"#
            ),
            "/blog/wp-json/",
            "rest_root case linked"
        );
        assert_eq!(
            rest_root("<html></html>"),
            "/wp-json/",
            "rest_root case default"
        );
    }
}
