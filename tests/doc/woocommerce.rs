// Shared by the `woocommerce` doc examples, which pull it in with a hidden
// `# mod support { include!("../tests/doc/woocommerce.rs"); }` line, so each
// example can run against a mock shop serving pages captured from the WooCommerce
// demo shop (examples/wordpress-sites/capture-fixtures.sh). It is not a test
// target of its own.
use goose::config::GooseConfiguration;
use goose::goose::get_base_url;
use goose::prelude::*;
use goose_eggs::woocommerce::{Shop, StoreApiCart};
use httpmock::Method::{GET, POST};
use httpmock::MockServer;
use std::future::Future;

const SHOP: &str = include_str!("../fixtures/woocommerce/shop.html");
const PRODUCT: &str = include_str!("../fixtures/woocommerce/product.html");
const PRODUCT_SEARCH: &str = include_str!("../fixtures/woocommerce/product-search.html");
const SHOP_BLOCK: &str = include_str!("../fixtures/woocommerce/shop-block.html");
const SHOP_PAGE_2: &str = include_str!("../fixtures/woocommerce/shop-block-page-2.html");
const CART_READ: &str = include_str!("../fixtures/woocommerce/store-cart-read.json");
const CART_READ_HEADERS: &str = include_str!("../fixtures/woocommerce/store-cart-read.json.headers");
const PRODUCT_BLOCK: &str = include_str!("../fixtures/woocommerce/product-block.html");
const PRODUCT_ADDED: &str = include_str!("../fixtures/woocommerce/product-added.html");
const BATCH_ADD: &str = include_str!("../fixtures/woocommerce/store-batch-add.json");
const CART_BLOCK: &str = include_str!("../fixtures/woocommerce/cart-block.html");
const UPDATE_ITEM_BLOCK: &str = include_str!("../fixtures/woocommerce/store-update-item-block.json");
const BATCH_REMOVE: &str = include_str!("../fixtures/woocommerce/store-batch-remove.json");
const CHECKOUT_BLOCK: &str = include_str!("../fixtures/woocommerce/checkout-block.html");
const BATCH_CUSTOMER: &str = include_str!("../fixtures/woocommerce/store-batch-customer.json");
const CHECKOUT_BLOCK_ORDER: &str = include_str!("../fixtures/woocommerce/store-checkout-block.json");
const ORDER_RECEIVED_BLOCK: &str = include_str!("../fixtures/woocommerce/order-received-block.html");
const CART: &str = include_str!("../fixtures/woocommerce/store-cart.json");
const CART_HEADERS: &str = include_str!("../fixtures/woocommerce/store-cart.json.headers");
const ADD_ITEM: &str = include_str!("../fixtures/woocommerce/store-add-item.json");
const UPDATE_ITEM: &str = include_str!("../fixtures/woocommerce/store-update-item.json");
const CHECKOUT: &str = include_str!("../fixtures/woocommerce/store-checkout.json");
const CLASSIC_ADD: &str = include_str!("../fixtures/woocommerce/classic-add-to-cart.html");
const CLASSIC_CART: &str = include_str!("../fixtures/woocommerce/classic-cart.html");
const CLASSIC_UPDATED: &str = include_str!("../fixtures/woocommerce/classic-cart-updated.html");
const CLASSIC_CHECKOUT: &str = include_str!("../fixtures/woocommerce/classic-checkout.html");
const CLASSIC_SUCCESS: &str = include_str!("../fixtures/woocommerce/classic-checkout-success.json");
const ORDER_RECEIVED: &str = include_str!("../fixtures/woocommerce/order-received.html");
const MY_ACCOUNT_LOGIN: &str = include_str!("../fixtures/woocommerce/my-account-login.html");
const MY_ACCOUNT: &str = include_str!("../fixtures/woocommerce/my-account.html");
const ORDERS: &str = include_str!("../fixtures/woocommerce/my-account-orders.html");

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

/// A response header saved with the Store API cart fixture.
fn cart_header(name: &str) -> String {
    CART_HEADERS
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{}: ", name)))
        .unwrap_or_else(|| panic!("no {} header saved", name))
        .to_string()
}

/// The nonce the Store API answered the cart fixture with.
pub fn nonce() -> String {
    cart_header("Nonce")
}

/// The cart token the Store API answered the cart fixture with.
pub fn cart_token() -> String {
    cart_header("Cart-Token")
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

// Panic naming `example` unless `result` passed.
fn passed<T>(example: &str, what: &str, result: Result<T, Box<TransactionError>>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("doc example {}: {} failed: {}", example, what, failure(&result)),
    }
}

/// A mock server and the mock whose hits an example counts.
pub struct Site {
    server: MockServer,
    counted: Vec<usize>,
}

impl Site {
    fn hits(&self, index: usize) -> usize {
        httpmock::Mock::new(self.counted[index], &self.server).hits()
    }
}

impl std::ops::Deref for Site {
    type Target = MockServer;
    fn deref(&self) -> &MockServer {
        &self.server
    }
}

/// A shop with its shop page at `/{path}`.
pub fn shop_site(path: &str) -> MockServer {
    let server = MockServer::start();
    mock_cart_read(&server);
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(SHOP);
    });
    server.mock(|when, then| {
        when.method(GET).path("/shop/page/2/");
        then.status(200).body(SHOP_PAGE_2);
    });
    server
}

/// Load the shop page of `shop`; panic naming `example` on failure.
pub fn shop_page(example: &str, server: &MockServer, shop: &Shop) {
    let mut user = user(server);
    passed(example, "shop page", block_on(goose_eggs::woocommerce::shop_page(&mut user, shop, None)));
}

/// A shop with a product page at `path` and the shop page at `/shop/`.
pub fn product_site(path: &str) -> MockServer {
    let server = shop_site("shop/");
    server.mock(|when, then| {
        when.method(GET).path(path);
        then.status(200).body(PRODUCT);
    });
    server
}

/// A shop answering a product search for `keys`.
pub fn product_search_site(keys: &str) -> Site {
    let server = MockServer::start();
    mock_cart_read(&server);
    let search = server.mock(|when, then| {
        when.method(GET).path("/").query_param("s", keys).query_param("post_type", "product");
        then.status(200).body(PRODUCT_SEARCH);
    });
    let counted = vec![search.id];
    Site { server, counted }
}

/// Searches made with the expected parameters.
pub fn searches(site: &Site) -> usize {
    site.hits(0)
}


// The Store API nonce a block cart or checkout fixture carries.
fn page_nonce(html: &str) -> String {
    regex::Regex::new(r"storeApiNonce: '([^']+)'")
        .unwrap()
        .captures(html)
        .unwrap()[1]
        .to_string()
}

// The Store API cart a page's product buttons read, answering with its nonce.
fn mock_cart_read(server: &MockServer) {
    let nonce = CART_READ_HEADERS
        .lines()
        .find_map(|line| line.strip_prefix("Nonce: "))
        .unwrap()
        .to_string();
    server.mock(|when, then| {
        when.method(GET).path("/wp-json/wc/store/v1/cart");
        then.status(200)
            .header("content-type", "application/json")
            .header("Nonce", nonce)
            .body(CART_READ);
    });
}

/// A shop whose product at `path` (the beanie, product 14) adds `quantity`
/// when its form is posted. Only the post is counted, as `adds`.
pub fn add_to_cart_site(path: &str, id: u64, quantity: u64) -> Site {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(path);
        then.status(200).body(PRODUCT_BLOCK);
    });
    mock_cart_read(&server);
    let add = server.mock(|when, then| {
        when.method(POST)
            .path("/product/beanie/")
            .x_www_form_urlencoded_tuple("add-to-cart", id.to_string())
            .x_www_form_urlencoded_tuple("quantity", quantity.to_string());
        then.status(200).body(PRODUCT_ADDED);
    });
    let counted = vec![add.id];
    Site { server, counted }
}

/// A shop page at `/{path}` whose Add to cart button for product `id` adds it
/// with the nonce the page's cart read answered with. Counted as `adds`.
pub fn archive_add_site(path: &str, id: u64) -> Site {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(SHOP_BLOCK);
    });
    mock_cart_read(&server);
    let nonce = CART_READ_HEADERS
        .lines()
        .find_map(|line| line.strip_prefix("Nonce: "))
        .unwrap()
        .to_string();
    let add = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .header("Nonce", nonce)
            .json_body_partial(format!(r#"{{"requests":[{{"body":{{"id":{}}}}}]}}"#, id));
        then.status(207).header("content-type", "application/json").body(BATCH_ADD);
    });
    let counted = vec![add.id];
    Site { server, counted }
}

/// The block cart at `/{path}`, holding the beanie, with its quantity and
/// remove controls on the Store API at `/{store_api}`.
pub fn cart_page_site_at(path: &str, store_api: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(CART_BLOCK);
    });
    let nonce = page_nonce(CART_BLOCK);
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}cart/update-item", store_api))
            .query_param("_locale", "site")
            .header("Nonce", nonce.as_str());
        then.status(200).header("content-type", "application/json").body(UPDATE_ITEM_BLOCK);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}batch", store_api))
            .query_param("_locale", "site")
            .header("Nonce", nonce.as_str());
        then.status(207).header("content-type", "application/json").body(BATCH_REMOVE);
    });
    server
}

/// The block cart at `/{path}`, on the default Store API.
pub fn cart_page_site(path: &str) -> MockServer {
    cart_page_site_at(path, "wp-json/wc/store/v1/")
}

/// Load the cart of `shop` and raise the beanie to 2; panic naming `example`
/// on failure.
pub fn update_cart_item(example: &str, server: &MockServer, shop: &Shop) {
    let mut user = user(server);
    let cart = passed(example, "cart_page", block_on(goose_eggs::woocommerce::cart_page(&mut user, shop)));
    passed(
        example,
        "update_cart_item",
        block_on(goose_eggs::woocommerce::update_cart_item(&mut user, shop, &cart, 14, 2)),
    );
}

/// The block checkout at `/{path}`.
pub fn checkout_page_site(path: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(CHECKOUT_BLOCK);
    });
    server
}

/// The block checkout, taking the typed address and the order with its nonce,
/// then showing the order received page. The order is counted as `checkouts`.
pub fn place_order_site() -> Site {
    let server = checkout_page_site("checkout/");
    let nonce = page_nonce(CHECKOUT_BLOCK);
    let update = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .header("Nonce", nonce.as_str())
            .json_body_partial(r#"{"requests":[{"path":"/wc/store/v1/cart/update-customer"}]}"#);
        then.status(207).header("content-type", "application/json").body(BATCH_CUSTOMER);
    });
    let checkout = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/checkout")
            .query_param("_locale", "site")
            .header("Nonce", nonce.as_str())
            .json_body_partial(r#"{"payment_method":"cod"}"#);
        then.status(200).header("content-type", "application/json").body(CHECKOUT_BLOCK_ORDER);
    });
    server.mock(|when, then| {
        when.method(GET).path_contains("/checkout/order-received/");
        then.status(200).body(ORDER_RECEIVED_BLOCK);
    });
    let counted = vec![update.id, checkout.id];
    Site { server, counted }
}

// The Store API cart routes, writes only answered with the cart's nonce (or,
// with `token`, only with its cart token).
fn cart_site(token: bool) -> Site {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/wp-json/wc/store/v1/cart");
        then.status(200)
            .header("content-type", "application/json")
            .header("Nonce", nonce())
            .header("Cart-Token", cart_token())
            .body(CART);
    });
    let write = |route: &str, body: &'static str| {
        server.mock(|when, then| {
            let when = when.method(POST).path(format!("/wp-json/wc/store/v1/{}", route));
            if token {
                when.header("Cart-Token", cart_token());
            } else {
                when.header("Nonce", nonce());
            }
            then.status(200)
                .header("content-type", "application/json")
                .header("Nonce", nonce())
                .body(body);
        })
    };
    let add = write("cart/add-item", ADD_ITEM);
    write("cart/update-item", UPDATE_ITEM);
    write("cart/remove-item", CART);
    let checkout = write("checkout", CHECKOUT);
    let counted = vec![add.id, checkout.id];
    Site { server, counted }
}

/// A Store API cart that takes writes with its nonce.
pub fn store_cart_site() -> Site {
    cart_site(false)
}

/// A Store API cart that takes writes with its cart token.
pub fn store_cart_token_site() -> Site {
    cart_site(true)
}

/// Items added with the nonce.
pub fn adds(site: &Site) -> usize {
    site.hits(0)
}

/// Checkouts made with the nonce.
pub fn checkouts(site: &Site) -> usize {
    site.hits(1)
}

/// Load `cart`; panic naming `example` on failure.
pub fn cart_load(example: &str, site: &Site, cart: &mut StoreApiCart) {
    let mut user = user(site);
    passed(example, "load", block_on(cart.load(&mut user)));
}

/// Add two of product 31 to `cart`; panic naming `example` unless it was added.
pub fn cart_add(example: &str, site: &Site, cart: &mut StoreApiCart) {
    let mut user = user(site);
    passed(example, "add_item", block_on(cart.add_item(&mut user, 31, 2)));
    if site.hits(0) != 1 {
        panic!("doc example {}: item added {} times, expected 1", example, site.hits(0));
    }
}

/// A classic shop adding `quantity` of product `id` at `/{cart}`.
pub fn classic_add_site(cart: &str, id: u64, quantity: u64) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path(format!("/{}", cart))
            .query_param("add-to-cart", id.to_string())
            .query_param("quantity", quantity.to_string());
        then.status(200).body(CLASSIC_ADD);
    });
    server
}

/// A classic cart at `/{cart}` that sets its line to 3 when posted with its nonce.
pub fn classic_cart_site(cart: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", cart));
        then.status(200).body(CLASSIC_CART);
    });
    let key = regex::Regex::new(r#"name="(cart\[[a-f0-9]+\]\[qty\])""#)
        .unwrap()
        .captures(CLASSIC_CART)
        .unwrap()[1]
        .to_string();
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}", cart))
            .x_www_form_urlencoded_tuple("woocommerce-cart-nonce", fixture_value(CLASSIC_CART, "woocommerce-cart-nonce"))
            .x_www_form_urlencoded_tuple(&key, "3")
            .x_www_form_urlencoded_key_exists("update_cart");
        then.status(200).body(CLASSIC_UPDATED);
    });
    server
}

/// Set the classic cart of `shop` to 3; panic naming `example` on failure.
pub fn classic_update_cart(example: &str, server: &MockServer, shop: &Shop) {
    let mut user = user(server);
    passed(
        example,
        "classic_update_cart",
        block_on(goose_eggs::woocommerce::classic_update_cart(&mut user, shop, 3)),
    );
}

/// A classic checkout at `/{checkout}` that places an order paid with
/// `payment_method` when posted with its nonce.
pub fn classic_checkout_site(checkout: &str, payment_method: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", checkout));
        then.status(200).body(CLASSIC_CHECKOUT);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .query_param("wc-ajax", "checkout")
            .x_www_form_urlencoded_tuple(
                "woocommerce-process-checkout-nonce",
                fixture_value(CLASSIC_CHECKOUT, "woocommerce-process-checkout-nonce"),
            )
            .x_www_form_urlencoded_tuple("payment_method", payment_method)
            .x_www_form_urlencoded_tuple("billing_email", "goose@example.com");
        then.status(200).header("content-type", "application/json").body(CLASSIC_SUCCESS);
    });
    server.mock(|when, then| {
        when.method(GET).path_contains("/checkout/order-received/");
        then.status(200).body(ORDER_RECEIVED);
    });
    server
}

/// Check out the classic cart of `shop`; panic naming `example` on failure.
pub fn classic_checkout(example: &str, server: &MockServer, shop: &Shop) {
    let mut user = user(server);
    let checkout = goose_eggs::woocommerce::Checkout::builder().build();
    passed(
        example,
        "classic_checkout",
        block_on(goose_eggs::woocommerce::classic_checkout(&mut user, shop, &checkout)),
    );
}

/// My Account at `/{path}`, logging in `username`:`password` with its nonce.
pub fn my_account_site(path: &str, username: &str, password: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(MY_ACCOUNT_LOGIN);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path(format!("/{}", path))
            .x_www_form_urlencoded_tuple("username", username)
            .x_www_form_urlencoded_tuple("password", password)
            .x_www_form_urlencoded_tuple(
                "woocommerce-login-nonce",
                fixture_value(MY_ACCOUNT_LOGIN, "woocommerce-login-nonce"),
            );
        then.status(200).body(MY_ACCOUNT);
    });
    server
}

/// Log into My Account of `shop`; panic naming `example` on failure.
pub fn my_account_log_in(example: &str, server: &MockServer, shop: &Shop, username: &str, password: &str) {
    let mut user = user(server);
    passed(
        example,
        "my_account_log_in",
        block_on(goose_eggs::woocommerce::my_account_log_in(&mut user, shop, username, password)),
    );
}

/// A logged in customer's orders at `/{path}`.
pub fn orders_site(path: &str) -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(200).body(ORDERS);
    });
    server
}

/// Load the orders of `shop`; panic naming `example` on failure.
pub fn my_account_orders(example: &str, server: &MockServer, shop: &Shop) {
    let mut user = user(server);
    passed(example, "my_account_orders", block_on(goose_eggs::woocommerce::my_account_orders(&mut user, shop)));
}
