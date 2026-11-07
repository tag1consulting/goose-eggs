//! Parity: every `wordpress` and `woocommerce` helper makes the requests a real
//! browser makes for the same action, as recorded in the HARs under
//! tests/fixtures/browser/ (see its README.md).
//!
//! Each helper runs against a demo site with Goose's request log on, and its
//! requests are compared with its action's HAR:
//!
//! - Pages and background requests (documents and XHRs), in order, by method
//!   and path. Numbers, and the values of `_wpnonce`, `_ajax_nonce`, `_nonce`
//!   and `key`, are masked, as they vary between runs. A redirect Goose follows
//!   counts as the request and the GET it led to, as a browser makes them.
//! - Static assets on the site, as a set.
//!
//! Any extra or missing request fails, named. A few differences are not a
//! helper's to remove and are allowed, each listed by the test as it runs:
//!
//! - An asset the browser loaded that no tag on the page names (a font or
//!   image named only by CSS, a script loaded by another script): out of reach
//!   without running the page's CSS and scripts.
//! - A lazy loaded image, and a `srcset` variant in place of an image's `src`:
//!   which a browser loads depends on its viewport.
//! - WordPress's Heartbeat (`admin-ajax.php` `action=heartbeat`), which the
//!   editor sends on a timer, not on an action.
//! - What a helper checks beyond the browser: `create_post` and `update_post`
//!   load the saved post's page to check its title.
//!
//! It needs the demo sites, so it does nothing unless `WORDPRESS_SITE` (the
//! theme test data site) or `WOOCOMMERCE_SITE` (the shop) is set. It is not in
//! tests/sabotage/LEDGER.tsv, whose breaks run offline; its proof is its CI
//! jobs, `wordpress` and `woocommerce`.
//!
//!   examples/wordpress-sites/theme-test-data/setup.sh start
//!   WORDPRESS_SITE=http://localhost:18081 cargo test --test parity_site -- --nocapture
//!   examples/wordpress-sites/woocommerce/setup.sh start
//!   WOOCOMMERCE_SITE=http://localhost:18082 cargo test --test parity_site -- --nocapture
use goose::client::GooseClientBuilder;
use goose::config::GooseConfiguration;
use goose::prelude::*;
use goose_eggs::woocommerce::{self, Checkout, Product, Shop};
use goose_eggs::wordpress::{self, Comment, Login, Post, SearchParams};
use regex::Regex;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/browser");

// A request in the browser's recording.
#[derive(Debug)]
struct Recorded {
    method: String,
    url: String,
    status: u64,
    kind: String,
    post_data: String,
}

// An action's recording: its requests, and the documents the browser loaded.
struct Har {
    base: String,
    // The User-Agent the browser sent.
    user_agent: String,
    entries: Vec<Recorded>,
    documents: Vec<String>,
}

fn har(site: &str, action: &str) -> Har {
    let path = format!("{}/{}/{}.har", FIXTURES, site, action);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let comment = json["log"]["comment"].as_str().unwrap_or_default();
    let base = Regex::new(r"Captured from (\S+)\.$")
        .unwrap()
        .captures(comment)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let user_agent = json["log"]["entries"][0]["request"]["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| {
            h["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case("user-agent"))
        })
        .and_then(|h| h["value"].as_str())
        .unwrap_or_default()
        .to_string();
    let mut entries = Vec::new();
    let mut documents = Vec::new();
    for entry in json["log"]["entries"].as_array().unwrap() {
        let kind = entry["_kind"].as_str().unwrap_or_default().to_string();
        if kind == "document" {
            if let Some(body) = entry["response"]["content"]["text"].as_str() {
                documents.push(body.to_string());
            }
        }
        entries.push(Recorded {
            method: entry["request"]["method"].as_str().unwrap().to_string(),
            url: entry["request"]["url"].as_str().unwrap().to_string(),
            status: entry["response"]["status"].as_u64().unwrap_or(0),
            kind,
            post_data: entry["request"]["postData"]["text"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        });
    }
    Har {
        base,
        user_agent,
        entries,
        documents,
    }
}

// The path and query of a URL, without its fragment.
fn path_of(url: &str) -> String {
    let path = match Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*://[^/]*")
        .unwrap()
        .find(url)
    {
        Some(origin) => &url[origin.end()..],
        None => url,
    };
    let path = path.split('#').next().unwrap_or_default();
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

// A path with what varies between runs masked.
fn masked(path: &str) -> String {
    let values = Regex::new(r"([?&](?:_wpnonce|_ajax_nonce|_nonce|key)=)[^&]*").unwrap();
    let path = values.replace_all(path, "${1}{v}");
    Regex::new(r"\d+")
        .unwrap()
        .replace_all(&path, "{n}")
        .into_owned()
}

fn on_site(url: &str, base: &str) -> bool {
    url.starts_with(base)
}

// A request Goose logged.
#[derive(Debug)]
struct Logged {
    transaction: String,
    name: String,
    method: String,
    url: String,
    final_url: String,
    redirected: bool,
}

fn read_log(path: &str) -> Vec<Logged> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .map(|json| Logged {
            // {"TransactionOnly": "<name>"} or {"InheritNameByRequests": "<name>"}.
            transaction: json["transaction_name"]
                .as_object()
                .and_then(|name| name.values().next())
                .or(Some(&json["transaction_name"]))
                .and_then(|name| name.as_str())
                .unwrap_or_default()
                .to_string(),
            name: json["name"].as_str().unwrap_or_default().to_string(),
            method: json["raw"]["method"]
                .as_str()
                .unwrap_or_default()
                .to_ascii_uppercase(),
            url: json["raw"]["url"].as_str().unwrap_or_default().to_string(),
            final_url: json["final_url"].as_str().unwrap_or_default().to_string(),
            redirected: json["redirected"].as_bool().unwrap_or(false),
        })
        .collect()
}

type Run = Arc<
    dyn for<'r> Fn(
            &'r mut GooseUser,
        ) -> Pin<Box<dyn Future<Output = TransactionResult> + Send + 'r>>
        + Send
        + Sync,
>;

// One step of a run: an action compared with its HAR, or setup that is not.
struct Step {
    name: &'static str,
    compare: bool,
    // Requests the helper makes after the browser's, on purpose: (method, path).
    after: Vec<(&'static str, &'static str)>,
    // The name of the first of those requests: the assets from it on are the
    // helper's own check's, not compared.
    after_name: Option<&'static str>,
    run: Run,
}

fn action<F>(name: &'static str, f: F) -> Step
where
    F: for<'r> Fn(
            &'r mut GooseUser,
        ) -> Pin<Box<dyn Future<Output = TransactionResult> + Send + 'r>>
        + Send
        + Sync
        + 'static,
{
    Step {
        name,
        compare: true,
        after: Vec::new(),
        after_name: None,
        run: Arc::new(f),
    }
}

fn setup<F>(name: &'static str, f: F) -> Step
where
    F: for<'r> Fn(
            &'r mut GooseUser,
        ) -> Pin<Box<dyn Future<Output = TransactionResult> + Send + 'r>>
        + Send
        + Sync
        + 'static,
{
    Step {
        name,
        compare: false,
        after: Vec::new(),
        after_name: None,
        run: Arc::new(f),
    }
}

// Run every step once, in order, as one user, logging every request. The user
// sends the browser's User-Agent: WordPress serves some pages by it (the
// editor leaves TinyMCE out for a user agent it does not know).
async fn run(site: &str, host: &str, steps: &[Step]) -> (GooseMetrics, Vec<Logged>) {
    let first = steps.iter().find(|s| s.compare).expect("a step to compare");
    let user_agent = har(site, first.name).user_agent;
    let log = std::env::temp_dir().join(format!(
        "goose-eggs-parity-{}-{}.log",
        std::process::id(),
        host.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
    ));
    let log = log.to_string_lossy().to_string();
    let _ = std::fs::remove_file(&log);
    let configuration = GooseConfiguration::parse_args_default(&[
        "--users",
        "1",
        "--increase-rate",
        "1",
        "--iterations",
        "1",
        "--host",
        host,
        "--co-mitigation",
        "disabled",
        "--no-telnet",
        "--no-websocket",
        "--quiet",
        "--no-print-metrics",
        "--request-log",
        &log,
        "--request-format",
        "json",
    ])
    .expect("a configuration");
    let mut scenario = scenario!("Parity");
    for step in steps {
        let f = step.run.clone();
        scenario = scenario.register_transaction(
            Transaction::new(Arc::new(move |user| f(user))).set_name(step.name),
        );
    }
    let metrics = GooseAttack::initialize_with_config(configuration)
        .unwrap()
        .set_client_builder_with_cookies(GooseClientBuilder::new().user_agent(user_agent))
        .unwrap()
        .register_scenario(scenario)
        .execute()
        .await
        .unwrap();
    let logged = read_log(&log);
    match env::var("PARITY_KEEP_LOG") {
        Ok(keep) => {
            let _ = std::fs::rename(&log, keep);
        }
        Err(_) => {
            let _ = std::fs::remove_file(&log);
        }
    }
    (metrics, logged)
}

// Tags whose attributes name assets, from markup with comments and script
// bodies removed: (tag, src or href, srcset candidates, lazy).
fn tag_assets(html: &str) -> Vec<(String, Vec<String>, bool)> {
    let html = Regex::new(r"(?is)<!--.*?-->|(<script\b[^>]*>).*?</script\s*>")
        .unwrap()
        .replace_all(html, |c: &regex::Captures| match c.get(1) {
            Some(open) => format!("{}</script>", open.as_str()),
            None => " ".to_string(),
        });
    let tag = Regex::new(r"(?is)<(?:script|link|img|source)\b[^>]*>").unwrap();
    let attr = |tag: &str, name: &str| -> Option<String> {
        Regex::new(&format!(r#"(?is)\s{}\s*=\s*(?:"([^"]*)"|'([^']*)')"#, name))
            .unwrap()
            .captures(tag)
            .and_then(|c| c.get(1).or_else(|| c.get(2)))
            .map(|m| html_escape::decode_html_entities(m.as_str()).to_string())
    };
    tag.find_iter(&html)
        .filter_map(|m| {
            let tag = m.as_str();
            let url = attr(tag, "src").or_else(|| attr(tag, "href"))?;
            let srcset = attr(tag, "srcset")
                .map(|s| {
                    s.split(',')
                        .filter_map(|c| c.split_whitespace().next())
                        .map(path_of)
                        .collect()
                })
                .unwrap_or_default();
            let lazy = attr(tag, "loading").is_some_and(|l| l == "lazy");
            Some((path_of(&url), srcset, lazy))
        })
        .collect()
}

// Compare one action's requests with its HAR; returns its problems and the
// differences allowed (named in the module docs), for the report.
fn compare(site: &str, step: &Step, logged: &[Logged]) -> (Vec<String>, Vec<String>) {
    let har = har(site, step.name);
    let mut problems = Vec::new();
    let mut allowed = Vec::new();

    let mut expected: Vec<String> = Vec::new();
    let mut back_to_same = false;
    for entry in &har.entries {
        if entry.kind == "asset" || !on_site(&entry.url, &har.base) {
            continue;
        }
        // A redirect back to the URL it came from: Goose follows it, but its
        // log calls a request redirected only when the URL changed.
        if back_to_same && entry.method == "GET" {
            back_to_same = false;
            allowed.push(format!("same URL redirect: GET {}", path_of(&entry.url)));
            continue;
        }
        back_to_same = false;
        if (300..400).contains(&entry.status) {
            let next = har
                .entries
                .iter()
                .skip_while(|e| !std::ptr::eq(*e, entry))
                .skip(1)
                .find(|e| e.kind != "asset");
            back_to_same = next.is_some_and(|n| path_of(&n.url) == path_of(&entry.url));
        }
        if entry.post_data.contains("action=heartbeat") {
            allowed.push(format!("timer: {} {}", entry.method, path_of(&entry.url)));
            continue;
        }
        expected.push(format!("{} {}", entry.method, masked(&path_of(&entry.url))));
    }
    for (method, path) in &step.after {
        expected.push(format!("{} {}", method, masked(path)));
    }
    let mine: Vec<&Logged> = logged
        .iter()
        .filter(|l| l.transaction == step.name)
        .collect();
    let mut actual: Vec<String> = Vec::new();
    for request in mine.iter().filter(|l| !l.name.starts_with("static asset:")) {
        actual.push(format!(
            "{} {}",
            request.method,
            masked(&path_of(&request.url))
        ));
        if request.redirected {
            actual.push(format!("GET {}", masked(&path_of(&request.final_url))));
        }
    }
    if expected != actual {
        problems.push(format!(
            "{}/{}: pages and background requests differ from the browser's\n    browser: {:#?}\n    helper:  {:#?}",
            site, step.name, expected, actual
        ));
    }

    let browser: BTreeSet<String> = har
        .entries
        .iter()
        .filter(|e| e.kind == "asset" && on_site(&e.url, &har.base))
        .map(|e| path_of(&e.url))
        .collect();
    let checked = match step.after_name {
        Some(name) => mine
            .iter()
            .position(|l| l.name == name)
            .unwrap_or(mine.len()),
        None => mine.len(),
    };
    let helper: BTreeSet<String> = mine[..checked]
        .iter()
        .filter(|l| l.name.starts_with("static asset:"))
        .map(|l| path_of(&l.url))
        .collect();
    let tags: Vec<(String, Vec<String>, bool)> =
        har.documents.iter().flat_map(|d| tag_assets(d)).collect();
    for url in browser.difference(&helper) {
        let named = tags
            .iter()
            .filter(|(src, set, _)| src == url || set.contains(url));
        let named: Vec<&(String, Vec<String>, bool)> = named.collect();
        if named.is_empty() {
            allowed.push(format!("out of reach: {}", url));
        } else if named.iter().any(|(_, _, lazy)| *lazy) {
            allowed.push(format!("lazy: {}", url));
        } else if named
            .iter()
            .any(|(src, set, _)| set.contains(url) && helper.contains(src))
        {
            allowed.push(format!("srcset variant: {}", url));
        } else {
            problems.push(format!("{}/{}: missing asset {}", site, step.name, url));
        }
    }
    for url in helper.difference(&browser) {
        let variant = tags.iter().any(|(src, set, _)| {
            src == url && set.iter().any(|candidate| browser.contains(candidate))
        });
        if variant {
            allowed.push(format!("srcset variant loaded as src: {}", url));
        } else {
            problems.push(format!("{}/{}: extra asset {}", site, step.name, url));
        }
    }
    (problems, allowed)
}

fn report(site: &str, steps: &[Step], metrics: &GooseMetrics, logged: &[Logged]) {
    let mut problems = Vec::new();
    for step in steps {
        let (ok, failed) = metrics
            .transactions
            .iter()
            .flatten()
            .find(|t| t.transaction_name.name_for_transaction() == step.name)
            .map(|t| (t.success_count, t.fail_count))
            .unwrap_or((0, 0));
        if (ok, failed) != (1, 0) {
            problems.push(format!(
                "{}/{}: expected one pass, got {} passed, {} failed",
                site, step.name, ok, failed
            ));
        }
        if !step.compare {
            continue;
        }
        let (mut wrong, allowed) = compare(site, step, logged);
        let documents = logged
            .iter()
            .filter(|l| l.transaction == step.name && !l.name.starts_with("static asset:"))
            .count();
        let assets = logged
            .iter()
            .filter(|l| l.transaction == step.name && l.name.starts_with("static asset:"))
            .count();
        println!(
            "parity {}/{}: {} ({} pages and background requests, {} assets; {} allowed differences)",
            site,
            step.name,
            if wrong.is_empty() { "ok" } else { "DIFFERS" },
            documents,
            assets,
            allowed.len()
        );
        for difference in &allowed {
            println!("    allowed {}", difference);
        }
        problems.append(&mut wrong);
    }
    let errors: HashMap<String, String> = metrics
        .errors
        .values()
        .map(|e| (e.name.clone(), e.error.clone()))
        .collect();
    if !errors.is_empty() {
        problems.push(format!("{}: request errors in the run: {:?}", site, errors));
    }
    assert!(
        problems.is_empty(),
        "{} parity problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}

fn editor() -> (String, String) {
    (
        env::var("EDITOR_USERNAME").unwrap_or_else(|_| "editor".to_string()),
        env::var("EDITOR_PASSWORD").unwrap_or_else(|_| "editor-password".to_string()),
    )
}

#[tokio::test]
async fn wordpress_parity() {
    let site = match env::var("WORDPRESS_SITE") {
        Ok(site) => site,
        Err(_) => {
            eprintln!("wordpress_parity: WORDPRESS_SITE is not set, nothing to test");
            return;
        }
    };
    let mut steps = vec![
        action("home", |user| {
            Box::pin(async move {
                wordpress::view_page(user, "", "home", None).await?;
                Ok(())
            })
        }),
        action("post", |user| {
            Box::pin(async move {
                wordpress::view_page(user, "hello-world/", "post", None).await?;
                Ok(())
            })
        }),
        action("page", |user| {
            Box::pin(async move {
                wordpress::view_page(user, "about/", "page", None).await?;
                Ok(())
            })
        }),
        action("category", |user| {
            Box::pin(async move {
                wordpress::view_page(user, "category/markup/", "category", None).await?;
                Ok(())
            })
        }),
        action("search", |user| {
            Box::pin(async move {
                let params = SearchParams::builder().keys("post").build();
                wordpress::search(user, &params).await?;
                Ok(())
            })
        }),
        action("log-in", |user| {
            Box::pin(async move {
                let (username, password) = editor();
                let login = Login::builder()
                    .username(username.as_str())
                    .password(password.as_str())
                    .build();
                wordpress::log_in(user, &login).await?;
                Ok(())
            })
        }),
        action("comment", |user| {
            Box::pin(async move {
                let comment = Comment::builder().post_url("hello-world/").build();
                wordpress::post_comment(user, &comment).await?;
                Ok(())
            })
        }),
        action("create-post", |user| {
            Box::pin(async move {
                let post = Post::builder().title("Parity draft").build();
                wordpress::create_post(user, &post).await?;
                Ok(())
            })
        }),
        action("update-post", |user| {
            Box::pin(async move {
                let title = format!("Hello world, parity {}", goose_eggs::text::random_word(6));
                let post = Post::builder().title(&title).build();
                wordpress::update_post(user, 1, &post).await?;
                Ok(())
            })
        }),
        action("log-out", |user| {
            Box::pin(async move {
                wordpress::log_out(user, "wp-admin/").await?;
                Ok(())
            })
        }),
    ];
    // The helpers' own check: the saved post's page, a draft's by its ID.
    steps[7].after = vec![("GET", "/?p=1")];
    steps[7].after_name = Some("post view");
    steps[8].after = vec![("GET", "/hello-world/")];
    steps[8].after_name = Some("post view");
    let (metrics, logged) = run("wordpress", &site, &steps).await;
    report("wordpress", &steps, &metrics, &logged);
}

static PRODUCTS: Mutex<Vec<Product>> = Mutex::new(Vec::new());
static CART: Mutex<String> = Mutex::new(String::new());
static CHECKOUT: Mutex<String> = Mutex::new(String::new());

fn product(name: &str) -> Product {
    PRODUCTS
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .unwrap_or_else(|| panic!("no {} in the shop", name))
}

fn customer() -> (String, String) {
    (
        env::var("CUSTOMER_USERNAME").unwrap_or_else(|_| "customer".to_string()),
        env::var("CUSTOMER_PASSWORD").unwrap_or_else(|_| "customer-password".to_string()),
    )
}

#[tokio::test]
async fn woocommerce_parity() {
    let site = match env::var("WOOCOMMERCE_SITE") {
        Ok(site) => site,
        Err(_) => {
            eprintln!("woocommerce_parity: WOOCOMMERCE_SITE is not set, nothing to test");
            return;
        }
    };
    // Two runs, each a new shopper with an empty cart, as each HAR starts: the
    // browsing and the add to cart from the shop page, then the buyer's way
    // from a product page through checkout and My Account.
    let browsing = vec![
        setup("discover products", |user| {
            Box::pin(async move {
                let products = woocommerce::discover_products(user, &Shop::default(), 100).await?;
                *PRODUCTS.lock().unwrap() = products;
                Ok(())
            })
        }),
        action("shop", |user| {
            Box::pin(async move {
                woocommerce::shop_page(user, &Shop::default(), None).await?;
                Ok(())
            })
        }),
        action("product-category", |user| {
            Box::pin(async move {
                wordpress::view_page(
                    user,
                    "product-category/accessories/",
                    "product category",
                    None,
                )
                .await?;
                Ok(())
            })
        }),
        action("product", |user| {
            Box::pin(async move {
                woocommerce::product_page(user, &product("Beanie").url, None).await?;
                Ok(())
            })
        }),
        action("add-to-cart-shop", |user| {
            Box::pin(async move {
                let hoodie = product("Hoodie with Logo");
                woocommerce::add_to_cart_from_archive(user, &Shop::default(), "shop/", &hoodie)
                    .await?;
                Ok(())
            })
        }),
    ];
    let buying = vec![
        action("add-to-cart-product-page", |user| {
            Box::pin(async move {
                woocommerce::add_to_cart(user, &product("Beanie"), 2).await?;
                Ok(())
            })
        }),
        // A second line, for remove-cart-item to take out, as its HAR's cart had.
        setup("add a cap", |user| {
            Box::pin(async move {
                woocommerce::add_to_cart(user, &product("Cap"), 1).await?;
                Ok(())
            })
        }),
        action("cart-page", |user| {
            Box::pin(async move {
                let html = woocommerce::cart_page(user, &Shop::default()).await?;
                *CART.lock().unwrap() = html;
                Ok(())
            })
        }),
        action("update-cart-item", |user| {
            Box::pin(async move {
                let cart = CART.lock().unwrap().clone();
                let beanie = product("Beanie").id;
                woocommerce::update_cart_item(user, &Shop::default(), &cart, beanie, 3).await?;
                Ok(())
            })
        }),
        action("remove-cart-item", |user| {
            Box::pin(async move {
                let cart = CART.lock().unwrap().clone();
                let cap = product("Cap").id;
                woocommerce::remove_cart_item(user, &Shop::default(), &cart, cap).await?;
                Ok(())
            })
        }),
        action("checkout-page", |user| {
            Box::pin(async move {
                let html = woocommerce::checkout_page(user, &Shop::default()).await?;
                *CHECKOUT.lock().unwrap() = html;
                Ok(())
            })
        }),
        action("place-order", |user| {
            Box::pin(async move {
                let page = CHECKOUT.lock().unwrap().clone();
                let checkout = Checkout::builder().build();
                woocommerce::place_order(user, &Shop::default(), &page, &checkout).await?;
                Ok(())
            })
        }),
        action("my-account-log-in", |user| {
            Box::pin(async move {
                let (username, password) = customer();
                woocommerce::my_account_log_in(user, &Shop::default(), &username, &password)
                    .await?;
                Ok(())
            })
        }),
        action("my-account-orders", |user| {
            Box::pin(async move {
                woocommerce::my_account_orders(user, &Shop::default()).await?;
                Ok(())
            })
        }),
    ];
    for steps in [browsing, buying] {
        let (metrics, logged) = run("woocommerce", &site, &steps).await;
        report("woocommerce", &steps, &metrics, &logged);
    }
}
