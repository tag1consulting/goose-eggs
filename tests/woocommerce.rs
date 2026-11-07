//! The `woocommerce` flows against pages and Store API responses captured from
//! the WooCommerce demo shop (examples/wordpress-sites/capture-fixtures.sh), in
//! one Goose run against one mock server. Each behaviour uses its own paths (its
//! own Store API root, for the cart), and each mock only answers the exact
//! fields, headers and nonces WooCommerce expects, so a wrong one shows as a 404.
mod common;

use common::{check_named, run, transaction, Expect};
use goose::prelude::*;
use goose_eggs::woocommerce::{self, Checkout, Product, Shop, StoreApiCart};
use goose_eggs::Validate;
use httpmock::prelude::HttpMockRequest;
use httpmock::{
    Method::{GET, POST},
    Mock, MockServer,
};

const SHOP: &str = include_str!("fixtures/woocommerce/shop.html");
const PRODUCT: &str = include_str!("fixtures/woocommerce/product.html");
const PRODUCT_SEARCH: &str = include_str!("fixtures/woocommerce/product-search.html");
const SHOP_BLOCK: &str = include_str!("fixtures/woocommerce/shop-block.html");
const SHOP_BLOCK_PAGE_2: &str = include_str!("fixtures/woocommerce/shop-block-page-2.html");
const STORE_CART_READ: &str = include_str!("fixtures/woocommerce/store-cart-read.json");
const STORE_CART_READ_HEADERS: &str =
    include_str!("fixtures/woocommerce/store-cart-read.json.headers");
const PRODUCT_BLOCK: &str = include_str!("fixtures/woocommerce/product-block.html");
const PRODUCT_ADDED: &str = include_str!("fixtures/woocommerce/product-added.html");
const PRODUCT_OUT_OF_STOCK: &str = include_str!("fixtures/woocommerce/product-out-of-stock.html");
const PRODUCT_ADD_OUT_OF_STOCK: &str =
    include_str!("fixtures/woocommerce/product-add-out-of-stock.html");
const STORE_BATCH_ADD: &str = include_str!("fixtures/woocommerce/store-batch-add.json");
const CART_BLOCK: &str = include_str!("fixtures/woocommerce/cart-block.html");
const STORE_UPDATE_ITEM_BLOCK: &str =
    include_str!("fixtures/woocommerce/store-update-item-block.json");
const STORE_BATCH_REMOVE: &str = include_str!("fixtures/woocommerce/store-batch-remove.json");
const CHECKOUT_BLOCK: &str = include_str!("fixtures/woocommerce/checkout-block.html");
const CHECKOUT_BLOCK_EMPTY: &str = include_str!("fixtures/woocommerce/checkout-block-empty.html");
const STORE_BATCH_CUSTOMER: &str = include_str!("fixtures/woocommerce/store-batch-customer.json");
const STORE_CHECKOUT_BLOCK: &str = include_str!("fixtures/woocommerce/store-checkout-block.json");
const ORDER_RECEIVED_BLOCK: &str = include_str!("fixtures/woocommerce/order-received-block.html");
const ORDER_RECEIVED_WRONG_KEY: &str =
    include_str!("fixtures/woocommerce/order-received-wrong-key.html");
const CART: &str = include_str!("fixtures/woocommerce/store-cart.json");
const ADD_ITEM: &str = include_str!("fixtures/woocommerce/store-add-item.json");
const ADD_ITEM_INVALID: &str = include_str!("fixtures/woocommerce/store-add-item-invalid.json");
const ADD_ITEM_OUT_OF_STOCK: &str =
    include_str!("fixtures/woocommerce/store-add-item-out-of-stock.json");
const ADD_ITEM_MISSING_NONCE: &str =
    include_str!("fixtures/woocommerce/store-add-item-missing-nonce.json");
const UPDATE_ITEM: &str = include_str!("fixtures/woocommerce/store-update-item.json");
const REMOVE_ITEM_INVALID: &str = include_str!("fixtures/woocommerce/store-remove-item.json");
const CHECKOUT: &str = include_str!("fixtures/woocommerce/store-checkout.json");
const CHECKOUT_INVALID: &str = include_str!("fixtures/woocommerce/store-checkout-invalid.json");
const CLASSIC_ADD: &str = include_str!("fixtures/woocommerce/classic-add-to-cart.html");
const CLASSIC_ADD_INVALID: &str =
    include_str!("fixtures/woocommerce/classic-add-to-cart-invalid.html");
const CLASSIC_CART: &str = include_str!("fixtures/woocommerce/classic-cart.html");
const CLASSIC_UPDATED: &str = include_str!("fixtures/woocommerce/classic-cart-updated.html");
const CLASSIC_CHECKOUT: &str = include_str!("fixtures/woocommerce/classic-checkout.html");
const CLASSIC_SUCCESS: &str = include_str!("fixtures/woocommerce/classic-checkout-success.json");
const CLASSIC_FAILURE: &str = include_str!("fixtures/woocommerce/classic-checkout-failure.json");
const ORDER_RECEIVED: &str = include_str!("fixtures/woocommerce/order-received.html");
const MY_ACCOUNT_LOGIN: &str = include_str!("fixtures/woocommerce/my-account-login.html");
const MY_ACCOUNT_FAILED: &str = include_str!("fixtures/woocommerce/my-account-login-failed.html");
const MY_ACCOUNT: &str = include_str!("fixtures/woocommerce/my-account.html");
const ORDERS: &str = include_str!("fixtures/woocommerce/my-account-orders.html");

const SHOP_PAGE: &str = "shop_page loads the product archive";
const SHOP_PAGE_WRONG: &str = "shop_page fails on a page that is not the archive";
const SHOP_PAGE_VALIDATION: &str = "shop_page fails a custom validation";
const PRODUCT_PAGE: &str = "product_page loads a single product";
const PRODUCT_PAGE_WRONG: &str = "product_page fails on a page that is not a product";
const PRODUCT_SEARCH_KEYS: &str = "product_search sends encoded keys for products";
const DISCOVER: &str = "discover_products reads the cards on every shop page";
const DISCOVER_CAP: &str = "discover_products stops at max";
const DISCOVER_STATUS: &str = "discover_products fails on an error status";
const DISCOVER_EMPTY: &str = "discover_products fails on a shop with no products";
const ADD_TO_CART: &str = "add_to_cart loads the product, then posts its form";
const ADD_TO_CART_NO_FORM: &str = "add_to_cart fails on a product out of stock";
const ADD_TO_CART_REFUSED: &str = "add_to_cart fails on WooCommerce's error notice";
const ADD_TO_CART_OPTIONS: &str = "add_to_cart fails on a product that needs a choice";
const ARCHIVE_ADD: &str = "add_to_cart_from_archive sends the button's batch with the read nonce";
const ARCHIVE_NO_BUTTON: &str = "add_to_cart_from_archive fails without the product's button";
const ARCHIVE_NO_NONCE: &str = "add_to_cart_from_archive fails without a nonce";
const ARCHIVE_REFUSED: &str = "add_to_cart_from_archive fails on a refused item";
const CART_PAGE: &str = "cart_page loads the block cart";
const UPDATE_CART_ITEM: &str = "update_cart_item sends the line's key with the page's nonces";
const UPDATE_NOT_IN_CART: &str = "update_cart_item fails on a product not in the cart";
const UPDATE_NO_NONCE: &str = "update_cart_item fails on a page without a nonce";
const UPDATE_STALE_NONCE: &str = "update_cart_item fails on a stale nonce";
const REMOVE_CART_ITEM: &str = "remove_cart_item sends the cart's batch";
const CHECKOUT_PAGE: &str = "checkout_page loads the block checkout";
const CHECKOUT_PAGE_WRONG: &str = "checkout_page fails on a page that is not the checkout";
const CART_PAGE_WRONG: &str = "cart_page fails on a page that is not the cart";
const PLACE_ORDER: &str = "place_order sends the typed address, then the order";
const PLACE_ORDER_ON_FILE: &str = "place_order sends no update for an address on file";
const PLACE_ORDER_EMPTY: &str = "place_order fails on an empty cart";
const PLACE_ORDER_NO_NONCE: &str = "place_order fails on a page without a nonce";
const PLACE_ORDER_INVALID: &str = "place_order fails on a validation error";
const PLACE_ORDER_NO_NUMBER: &str = "place_order fails without an order number";
const CART_LOAD: &str = "load keeps the nonce and cart token";
const CART_ADD: &str = "add_item loads the cart for a nonce, then sends it";
const CART_NONCE_REFRESH: &str = "update_item and remove_item send the latest nonce";
const CART_TOKEN: &str = "use_cart_token sends the cart token";
const CART_INVALID: &str = "add_item fails on a product that does not exist";
const CART_OUT_OF_STOCK: &str = "add_item fails on a product out of stock";
const CART_MISSING_NONCE: &str = "add_item fails when the Store API gave no nonce";
const CART_REMOVE_INVALID: &str = "remove_item fails on a line no longer in the cart";
const CHECKOUT_ORDER: &str = "checkout sends the address and payment method";
const CHECKOUT_CUSTOM: &str = "checkout sends custom billing and payment";
const CHECKOUT_INVALID_ADDRESS: &str = "checkout fails on a validation error";
const CHECKOUT_NOT_PAID: &str = "checkout fails when the payment did not succeed";
const CHECKOUT_NO_ORDER: &str = "checkout fails without an order";
const CLASSIC_ADD_TO_CART: &str = "classic_add_to_cart adds by ID and quantity";
const CLASSIC_ADD_NOTHING: &str = "classic_add_to_cart fails without a success notice";
const CLASSIC_UPDATE: &str = "classic_update_cart sets every line with the cart nonce";
const CLASSIC_UPDATE_IGNORED: &str = "classic_update_cart fails when not confirmed";
const CLASSIC_UPDATE_NO_FORM: &str = "classic_update_cart fails without a cart form";
const CLASSIC_CHECKOUT_ORDER: &str = "classic_checkout posts the form, then loads the order";
const CLASSIC_CHECKOUT_FAILED: &str = "classic_checkout fails on a failure result";
const CLASSIC_CHECKOUT_NO_FORM: &str = "classic_checkout fails without a checkout form";
const MY_ACCOUNT_LOG_IN: &str = "my_account_log_in posts the form with its nonce";
const MY_ACCOUNT_WRONG: &str = "my_account_log_in fails on a wrong password";
const MY_ACCOUNT_NO_FORM: &str = "my_account_log_in fails without a log in form";
const MY_ACCOUNT_ORDERS: &str = "my_account_orders loads the orders";
const MY_ACCOUNT_ORDERS_LOGGED_OUT: &str = "my_account_orders fails when not logged in";

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

// Serve `body` at `/{path}`.
fn page<'a>(server: &'a MockServer, path: &str, status: u16, body: &str) -> Mock<'a> {
    let body = body.to_string();
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}", path));
        then.status(status).body(body);
    })
}

// The JSON body of a request.
fn json(request: &HttpMockRequest) -> serde_json::Value {
    serde_json::from_slice(request.body.as_deref().unwrap_or_default()).unwrap_or_default()
}

// The checkout the default Checkout sends.
fn default_checkout(request: &HttpMockRequest) -> bool {
    let body = json(request);
    let billing = serde_json::json!({
        "first_name": "Goose", "last_name": "Shopper", "address_1": "1 Main Street",
        "city": "San Francisco", "state": "CA", "postcode": "94105", "country": "US",
        "email": "goose@example.com", "phone": "5555550100",
    });
    let mut shipping = billing.clone();
    shipping.as_object_mut().unwrap().remove("email");
    body == serde_json::json!({
        "billing_address": billing,
        "shipping_address": shipping,
        "payment_method": "cod",
    })
}

// A request without a Nonce header.
fn no_nonce(request: &HttpMockRequest) -> bool {
    !request
        .headers
        .as_ref()
        .is_some_and(|h| h.iter().any(|(k, _)| k.eq_ignore_ascii_case("nonce")))
}

// A checkout with custom billing and payment.
fn custom_checkout(request: &HttpMockRequest) -> bool {
    let body = json(request);
    body["billing_address"]["city"] == "Austin"
        && body["shipping_address"]["city"] == "Austin"
        && body["billing_address"]["company"] == "Tag1"
        && body["payment_method"] == "cheque"
}

// Answer `POST /{root}{route}` carrying `nonce` (or, with no nonce, no Nonce
// header) with `answer`: a status, a body and the `Nonce` header to send.
fn mock_store<'a>(
    server: &'a MockServer,
    root: &str,
    route: &str,
    nonce: Option<&str>,
    check: Option<fn(&HttpMockRequest) -> bool>,
    answer: (u16, &str, &str),
) -> Mock<'a> {
    let (status, body, answer_nonce) = answer;
    let body = body.to_string();
    let answer_nonce = answer_nonce.to_string();
    server.mock(|when, then| {
        let mut when = when
            .method(POST)
            .path(format!("/{}{}", root, route))
            .header("content-type", "application/json");
        when = match nonce {
            Some(nonce) => when.header("Nonce", nonce),
            None => when.matches(no_nonce),
        };
        if let Some(check) = check {
            when.matches(check);
        }
        then.status(status)
            .header("content-type", "application/json")
            .header("Nonce", answer_nonce)
            .body(body);
    })
}

// Answer `GET /{root}cart` with the cart, `nonce` and a cart token.
fn mock_cart<'a>(server: &'a MockServer, root: &str, nonce: Option<&str>) -> Mock<'a> {
    let nonce = nonce.map(str::to_string);
    server.mock(|when, then| {
        when.method(GET).path(format!("/{}cart", root));
        let then = then
            .status(200)
            .header("content-type", "application/json")
            .header(
                "Cart-Token",
                format!("token-{}", root.trim_end_matches('/')),
            )
            .body(CART);
        if let Some(nonce) = nonce {
            then.header("Nonce", nonce);
        }
    })
}

#[tokio::test]
async fn woocommerce_flows() {
    let server = MockServer::start();

    // Browsing.
    page(&server, "shop/", 200, SHOP_BLOCK);
    page(&server, "validshop/", 200, SHOP);
    page(&server, "wrongshop/", 200, PRODUCT);
    page(&server, "product/beanie/", 200, PRODUCT);
    let search = server.mock(|when, then| {
        when.method(GET)
            .path("/")
            .query_param("s", "logo & more")
            .query_param("post_type", "product");
        then.status(200).body(PRODUCT_SEARCH);
    });

    // Product discovery from the shop's cards: two pages, a cap, an error, a
    // shop with no products.
    page(&server, "cards/", 200, SHOP_BLOCK);
    page(&server, "capcards/", 200, SHOP_BLOCK);
    let shop_page_2 = page(&server, "shop/page/2/", 200, SHOP_BLOCK_PAGE_2);
    page(&server, "brokenshop/", 500, "down");
    page(
        &server,
        "emptyshop/",
        200,
        "<html><body class=\"post-type-archive-product\"><ul></ul></body></html>",
    );

    // The Store API cart a page's product buttons read, with its nonce.
    let cart_read_nonce = header_of(STORE_CART_READ_HEADERS, "Nonce");
    let nonce = cart_read_nonce.clone();
    server.mock(|when, then| {
        when.method(GET).path("/wp-json/wc/store/v1/cart");
        then.status(200)
            .header("content-type", "application/json")
            .header("Nonce", nonce)
            .body(STORE_CART_READ);
    });
    server.mock(|when, then| {
        when.method(GET).path("/nononce/wp-json/wc/store/v1/cart");
        then.status(200)
            .header("content-type", "application/json")
            .body(STORE_CART_READ);
    });

    // Adding to the cart from a product page: the form posts to the product.
    page(&server, "added/beanie/", 200, PRODUCT_BLOCK);
    page(&server, "gone/cap/", 200, PRODUCT_OUT_OF_STOCK);
    page(&server, "refused/beanie/", 200, PRODUCT_BLOCK);
    let options = PRODUCT_BLOCK.replacen(
        r#"<button type="submit" name="add-to-cart""#,
        r#"<input type="hidden" name="variation_id" value="0" /><button type="submit" name="add-to-cart""#,
        1,
    );
    page(&server, "options/v-neck/", 200, &options);
    let product_add = server.mock(|when, then| {
        when.method(POST)
            .path("/product/beanie/")
            .x_www_form_urlencoded_tuple("add-to-cart", "14")
            .x_www_form_urlencoded_tuple("quantity", "2");
        then.status(200).body(PRODUCT_ADDED);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/product/beanie/")
            .x_www_form_urlencoded_tuple("quantity", "9");
        then.status(200).body(PRODUCT_ADD_OUT_OF_STOCK);
    });

    // Adding to the cart from the shop page's button: a Store API batch, after
    // the cart read the page's buttons make, counted on its own API root.
    let archive = SHOP_BLOCK.replace(r#"href="/wp-json/""#, r#"href="/archive-api/wp-json/""#);
    page(&server, "archive/", 200, &archive);
    let nonce = cart_read_nonce.clone();
    let archive_read = server.mock(|when, then| {
        when.method(GET)
            .path("/archive-api/wp-json/wc/store/v1/cart");
        then.status(200)
            .header("content-type", "application/json")
            .header("Nonce", nonce)
            .body(STORE_CART_READ);
    });
    page(
        &server,
        "nononce-archive/",
        200,
        &SHOP_BLOCK.replace(r#"href="/wp-json/""#, r#"href="/nononce/wp-json/""#),
    );
    let nonce = cart_read_nonce.clone();
    let archive_add = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .header("Nonce", nonce)
            .json_body_partial(r#"{"requests":[{"path":"/wc/store/v1/cart/add-item","body":{"id":12,"quantity":1}}]}"#);
        then.status(207).header("content-type", "application/json").body(STORE_BATCH_ADD);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .json_body_partial(r#"{"requests":[{"body":{"id":15}}]}"#);
        then.status(207).header("content-type", "application/json").body(
            r#"{"responses":[{"status":400,"body":{"code":"woocommerce_rest_product_out_of_stock","message":"You cannot add &quot;Cap&quot; to the cart because the product is out of stock."}}]}"#,
        );
    });

    // The block cart: its page, and its controls' Store API requests.
    let cart_nonce = store_nonce(CART_BLOCK);
    page(&server, "cart/", 200, CART_BLOCK);
    let nonce = cart_nonce.clone();
    let update_item = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/cart/update-item")
            .query_param("_locale", "site")
            .header("Nonce", nonce)
            .header_exists("X-WP-Nonce")
            .json_body_partial(r#"{"quantity":2}"#);
        then.status(200)
            .header("content-type", "application/json")
            .body(STORE_UPDATE_ITEM_BLOCK);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/cart/update-item")
            .header("Nonce", "stale0");
        then.status(403).header("content-type", "application/json").body(
            r#"{"code":"woocommerce_rest_invalid_nonce","message":"Nonce is invalid.","data":{"status":403}}"#,
        );
    });
    let nonce = cart_nonce.clone();
    let remove_item = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .query_param("_locale", "site")
            .header("Nonce", nonce)
            .json_body_partial(r#"{"requests":[{"path":"/wc/store/v1/cart/remove-item"}]}"#);
        then.status(207)
            .header("content-type", "application/json")
            .body(STORE_BATCH_REMOVE);
    });

    // The block checkout: its page, the customer updates, the order.
    page(&server, "checkout/", 200, CHECKOUT_BLOCK);
    page(&server, "notcheckout/", 200, PRODUCT_BLOCK);
    page(&server, "notcart/", 200, PRODUCT_BLOCK);
    let nonce = store_nonce(CHECKOUT_BLOCK);
    let customer_updates = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .query_param("_locale", "site")
            .header("Nonce", nonce)
            .json_body_partial(r#"{"requests":[{"path":"/wc/store/v1/cart/update-customer","body":{"billing_address":{"email":"goose@example.com"}}}]}"#);
        then.status(207).header("content-type", "application/json").body(STORE_BATCH_CUSTOMER);
    });
    let nonce = store_nonce(CHECKOUT_BLOCK);
    let place = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/checkout")
            .query_param("_locale", "site")
            .header("Nonce", nonce)
            .header_exists("X-WP-Nonce")
            .json_body_partial(r#"{"payment_method":"cod","billing_address":{"email":"goose@example.com","postcode":"94105"},"expected_total":"1800"}"#);
        then.status(200).header("content-type", "application/json").body(STORE_CHECKOUT_BLOCK);
    });
    let on_file = server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/checkout")
            .header("Nonce", "onfile0")
            .json_body_partial(r#"{"billing_address":{"email":"","city":""}}"#);
        then.status(200)
            .header("content-type", "application/json")
            .body(STORE_CHECKOUT_BLOCK);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/checkout")
            .header("Nonce", "invalid0");
        then.status(400)
            .header("content-type", "application/json")
            .body(CHECKOUT_INVALID);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/checkout")
            .header("Nonce", "nonumber0");
        then.status(200)
            .header("content-type", "application/json")
            .body(STORE_CHECKOUT_BLOCK.replace("order-received\\/40\\/", "order-received\\/41\\/"));
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/wp-json/wc/store/v1/batch")
            .header_exists("Nonce")
            .matches(|request| {
                request.headers.as_ref().is_some_and(|h| {
                    h.iter().any(|(k, v)| {
                        k.eq_ignore_ascii_case("Nonce")
                            && ["invalid0", "nonumber0"].contains(&v.as_str())
                    })
                })
            });
        then.status(207)
            .header("content-type", "application/json")
            .body(STORE_BATCH_CUSTOMER);
    });
    let block_received = page(
        &server,
        "checkout/order-received/40/",
        200,
        ORDER_RECEIVED_BLOCK,
    );
    page(
        &server,
        "checkout/order-received/41/",
        200,
        ORDER_RECEIVED_WRONG_KEY,
    );

    // The Store API cart, a root per behaviour.
    let load = mock_cart(&server, "load/", Some("n-load"));
    mock_cart(&server, "add/", Some("n-add"));
    let add = mock_store(
        &server,
        "add/",
        "cart/add-item",
        Some("n-add"),
        None,
        (201, ADD_ITEM, "n-add"),
    );
    mock_cart(&server, "refresh/", Some("n-first"));
    mock_store(
        &server,
        "refresh/",
        "cart/add-item",
        Some("n-first"),
        None,
        (201, ADD_ITEM, "n-second"),
    );
    let refreshed_update = mock_store(
        &server,
        "refresh/",
        "cart/update-item",
        Some("n-second"),
        None,
        (200, UPDATE_ITEM, "n-third"),
    );
    let refreshed_remove = mock_store(
        &server,
        "refresh/",
        "cart/remove-item",
        Some("n-third"),
        None,
        (200, CART, "n-third"),
    );
    mock_cart(&server, "token/", Some("n-token"));
    let token = server.mock(|when, then| {
        when.method(POST)
            .path("/token/cart/add-item")
            .header("Cart-Token", "token-token");
        then.status(201)
            .header("content-type", "application/json")
            .body(ADD_ITEM);
    });
    mock_cart(&server, "invalid/", Some("n-invalid"));
    mock_store(
        &server,
        "invalid/",
        "cart/add-item",
        Some("n-invalid"),
        None,
        (400, ADD_ITEM_INVALID, "n-invalid"),
    );
    mock_cart(&server, "stock/", Some("n-stock"));
    mock_store(
        &server,
        "stock/",
        "cart/add-item",
        Some("n-stock"),
        None,
        (400, ADD_ITEM_OUT_OF_STOCK, "n-stock"),
    );
    mock_cart(&server, "nononce/", None);
    mock_store(
        &server,
        "nononce/",
        "cart/add-item",
        None,
        None,
        (401, ADD_ITEM_MISSING_NONCE, "n-late"),
    );
    mock_cart(&server, "remove/", Some("n-remove"));
    mock_store(
        &server,
        "remove/",
        "cart/remove-item",
        Some("n-remove"),
        None,
        (409, REMOVE_ITEM_INVALID, "n-remove"),
    );
    let mut checkouts = Vec::new();
    for (root, check, status, body) in [
        (
            "checkout/",
            default_checkout as fn(&HttpMockRequest) -> bool,
            200,
            CHECKOUT.to_string(),
        ),
        ("custom/", custom_checkout, 200, CHECKOUT.to_string()),
        (
            "badaddress/",
            default_checkout,
            400,
            CHECKOUT_INVALID.to_string(),
        ),
        (
            "notpaid/",
            default_checkout,
            200,
            CHECKOUT.replace(
                r#""payment_status":"success""#,
                r#""payment_status":"failure""#,
            ),
        ),
        (
            "noorder/",
            default_checkout,
            200,
            r#"{"status":"checkout-draft"}"#.to_string(),
        ),
    ] {
        let nonce = format!("n-{}", root.trim_end_matches('/'));
        mock_cart(&server, root, Some(&nonce));
        checkouts.push(mock_store(
            &server,
            root,
            "checkout",
            Some(&nonce),
            Some(check),
            (status, &body, &nonce),
        ));
    }

    // Classic flows.
    let classic_add = server.mock(|when, then| {
        when.method(GET)
            .path("/classic-cart/")
            .query_param("add-to-cart", "31")
            .query_param("quantity", "2");
        then.status(200).body(CLASSIC_ADD);
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/classic-cart/")
            .query_param("add-to-cart", "999999");
        then.status(200).body(CLASSIC_ADD_INVALID);
    });
    let cart_nonce = fixture_value(CLASSIC_CART, "woocommerce-cart-nonce");
    let quantity = regex::Regex::new(r#"name="(cart\[[a-f0-9]+\]\[qty\])""#)
        .unwrap()
        .captures(CLASSIC_CART)
        .unwrap()[1]
        .to_string();
    for (cart, answer) in [("update/", CLASSIC_UPDATED), ("ignored/", CLASSIC_CART)] {
        page(&server, cart, 200, CLASSIC_CART);
        let answer = answer.to_string();
        server.mock(|when, then| {
            when.method(POST)
                .path(format!("/{}", cart))
                .x_www_form_urlencoded_tuple("woocommerce-cart-nonce", &cart_nonce)
                .x_www_form_urlencoded_tuple(&quantity, "0")
                .x_www_form_urlencoded_tuple("update_cart", "Update cart");
            then.status(200).body(answer);
        });
    }
    page(
        &server,
        "noform/",
        200,
        "<html><body>Your cart is empty</body></html>",
    );
    let checkout_nonce = fixture_value(CLASSIC_CHECKOUT, "woocommerce-process-checkout-nonce");
    page(&server, "classic-checkout/", 200, CLASSIC_CHECKOUT);
    page(
        &server,
        "failing-checkout/",
        200,
        &CLASSIC_CHECKOUT.replace(&checkout_nonce, "fail0fail0"),
    );
    let classic_checkout = server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .query_param("wc-ajax", "checkout")
            .x_www_form_urlencoded_tuple("woocommerce-process-checkout-nonce", &checkout_nonce)
            .x_www_form_urlencoded_tuple("billing_first_name", "Goose")
            .x_www_form_urlencoded_tuple("billing_email", "goose@example.com")
            .x_www_form_urlencoded_tuple("billing_country", "US")
            .x_www_form_urlencoded_tuple("shipping_method[0]", "free_shipping:1")
            .x_www_form_urlencoded_tuple("payment_method", "cod");
        then.status(200)
            .header("content-type", "application/json")
            .body(CLASSIC_SUCCESS);
    });
    server.mock(|when, then| {
        when.method(POST)
            .path("/")
            .query_param("wc-ajax", "checkout")
            .x_www_form_urlencoded_tuple("woocommerce-process-checkout-nonce", "fail0fail0");
        then.status(200)
            .header("content-type", "application/json")
            .body(CLASSIC_FAILURE);
    });
    let order_received = server.mock(|when, then| {
        when.method(GET).path("/checkout/order-received/63/");
        then.status(200).body(ORDER_RECEIVED);
    });

    // My Account.
    let login_nonce = fixture_value(MY_ACCOUNT_LOGIN, "woocommerce-login-nonce");
    for (path, password, answer) in [
        ("my-account/", "secret", MY_ACCOUNT),
        ("wrong-account/", "wrong", MY_ACCOUNT_FAILED),
    ] {
        page(&server, path, 200, MY_ACCOUNT_LOGIN);
        let answer = answer.to_string();
        server.mock(|when, then| {
            when.method(POST)
                .path(format!("/{}", path))
                .x_www_form_urlencoded_tuple("username", "customer")
                .x_www_form_urlencoded_tuple("password", password)
                .x_www_form_urlencoded_tuple("woocommerce-login-nonce", &login_nonce)
                .x_www_form_urlencoded_tuple("login", "Log in");
            then.status(200).body(answer);
        });
    }
    page(
        &server,
        "no-account/",
        200,
        "<html><body>Maintenance</body></html>",
    );
    page(&server, "my-account/orders/", 200, ORDERS);
    page(&server, "logged-out/orders/", 200, MY_ACCOUNT_LOGIN);

    let metrics = run(
        &server.base_url(),
        vec![
            transaction(SHOP_PAGE, |user| {
                Box::pin(async move {
                    woocommerce::shop_page(user, &Shop::default(), None).await?;
                    Ok(())
                })
            }),
            transaction(SHOP_PAGE_WRONG, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().shop("wrongshop/").build();
                    woocommerce::shop_page(user, &shop, None).await?;
                    Ok(())
                })
            }),
            transaction(SHOP_PAGE_VALIDATION, |user| {
                Box::pin(async move {
                    let validate = Validate::builder().title("Not the shop").build();
                    let shop = Shop::builder().shop("validshop/").build();
                    woocommerce::shop_page(user, &shop, Some(&validate)).await?;
                    Ok(())
                })
            }),
            transaction(PRODUCT_PAGE, |user| {
                Box::pin(async move {
                    woocommerce::product_page(user, "product/beanie/", None).await?;
                    Ok(())
                })
            }),
            transaction(PRODUCT_PAGE_WRONG, |user| {
                Box::pin(async move {
                    woocommerce::product_page(user, "shop/", None).await?;
                    Ok(())
                })
            }),
            transaction(PRODUCT_SEARCH_KEYS, |user| {
                Box::pin(async move {
                    woocommerce::product_search(user, "logo & more", None).await?;
                    Ok(())
                })
            }),
            transaction(DISCOVER, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().shop("cards/").build();
                    let products = woocommerce::discover_products(user, &shop, 500).await?;
                    let simple = products.iter().filter(|p| p.can_add_to_cart()).count();
                    let options = products.iter().filter(|p| p.has_options).count();
                    // 16 on the first page, the external product on the second.
                    if products.len() != 17 || products[16].id != 33 || simple == 0 || options == 0
                    {
                        return Err(wrong(format!("found {:?}", products)));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_CAP, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().shop("capcards/").build();
                    let products = woocommerce::discover_products(user, &shop, 3).await?;
                    if products.len() != 3 {
                        return Err(wrong(format!("found {}", products.len())));
                    }
                    Ok(())
                })
            }),
            transaction(DISCOVER_STATUS, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().shop("brokenshop/").build();
                    woocommerce::discover_products(user, &shop, 10).await?;
                    Ok(())
                })
            }),
            transaction(DISCOVER_EMPTY, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().shop("emptyshop/").build();
                    woocommerce::discover_products(user, &shop, 10).await?;
                    Ok(())
                })
            }),
            transaction(ADD_TO_CART, |user| {
                Box::pin(async move {
                    let html =
                        woocommerce::add_to_cart(user, &product(14, "/added/beanie/"), 2).await?;
                    if !html.contains("added to your cart") {
                        return Err(wrong("no added notice".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(ADD_TO_CART_NO_FORM, |user| {
                Box::pin(async move {
                    woocommerce::add_to_cart(user, &product(16, "/gone/cap/"), 1).await?;
                    Ok(())
                })
            }),
            transaction(ADD_TO_CART_REFUSED, |user| {
                Box::pin(async move {
                    woocommerce::add_to_cart(user, &product(14, "/refused/beanie/"), 9).await?;
                    Ok(())
                })
            }),
            transaction(ADD_TO_CART_OPTIONS, |user| {
                Box::pin(async move {
                    woocommerce::add_to_cart(user, &product(10, "/options/v-neck/"), 1).await?;
                    Ok(())
                })
            }),
            transaction(ARCHIVE_ADD, |user| {
                Box::pin(async move {
                    let cart = woocommerce::add_to_cart_from_archive(
                        user,
                        &Shop::default(),
                        "archive/",
                        &product(12, "/product/hoodie-with-logo/"),
                    )
                    .await?;
                    if cart["items"]
                        .as_array()
                        .is_none_or(|items| items.is_empty())
                    {
                        return Err(wrong(format!("cart {}", cart)));
                    }
                    Ok(())
                })
            }),
            transaction(ARCHIVE_NO_BUTTON, |user| {
                Box::pin(async move {
                    let vneck = product(10, "/product/v-neck-t-shirt/");
                    woocommerce::add_to_cart_from_archive(
                        user,
                        &Shop::default(),
                        "archive/",
                        &vneck,
                    )
                    .await?;
                    Ok(())
                })
            }),
            transaction(ARCHIVE_NO_NONCE, |user| {
                Box::pin(async move {
                    let hoodie = product(12, "/product/hoodie-with-logo/");
                    woocommerce::add_to_cart_from_archive(
                        user,
                        &Shop::default(),
                        "nononce-archive/",
                        &hoodie,
                    )
                    .await?;
                    Ok(())
                })
            }),
            transaction(ARCHIVE_REFUSED, |user| {
                Box::pin(async move {
                    let cap = product(15, "/product/cap/");
                    woocommerce::add_to_cart_from_archive(user, &Shop::default(), "archive/", &cap)
                        .await?;
                    Ok(())
                })
            }),
            transaction(CART_PAGE, |user| {
                Box::pin(async move {
                    woocommerce::cart_page(user, &Shop::default()).await?;
                    Ok(())
                })
            }),
            transaction(UPDATE_CART_ITEM, |user| {
                Box::pin(async move {
                    let cart =
                        woocommerce::update_cart_item(user, &Shop::default(), CART_BLOCK, 14, 2)
                            .await?;
                    if cart["items"][0]["quantity"] != 2 {
                        return Err(wrong(format!("cart {}", cart["items"])));
                    }
                    Ok(())
                })
            }),
            transaction(UPDATE_NOT_IN_CART, |user| {
                Box::pin(async move {
                    expect_custom(
                        woocommerce::update_cart_item(user, &Shop::default(), CART_BLOCK, 99, 2)
                            .await,
                        "cart/: product 99 is not in the cart",
                    )
                })
            }),
            transaction(UPDATE_NO_NONCE, |user| {
                Box::pin(async move {
                    let page = CART_BLOCK.replace("storeApiNonce", "storeApiNonceGone");
                    expect_custom(
                        woocommerce::update_cart_item(user, &Shop::default(), &page, 14, 2).await,
                        "cart/: no Store API nonce on the page",
                    )
                })
            }),
            transaction(UPDATE_STALE_NONCE, |user| {
                Box::pin(async move {
                    let page = with_store_nonce(CART_BLOCK, "stale0");
                    woocommerce::update_cart_item(user, &Shop::default(), &page, 14, 2).await?;
                    Ok(())
                })
            }),
            transaction(REMOVE_CART_ITEM, |user| {
                Box::pin(async move {
                    woocommerce::remove_cart_item(user, &Shop::default(), CART_BLOCK, 14).await?;
                    Ok(())
                })
            }),
            transaction(CART_PAGE_WRONG, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().cart("notcart/").build();
                    woocommerce::cart_page(user, &shop).await?;
                    Ok(())
                })
            }),
            transaction(CHECKOUT_PAGE_WRONG, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().checkout("notcheckout/").build();
                    woocommerce::checkout_page(user, &shop).await?;
                    Ok(())
                })
            }),
            transaction(CHECKOUT_PAGE, |user| {
                Box::pin(async move {
                    woocommerce::checkout_page(user, &Shop::default()).await?;
                    Ok(())
                })
            }),
            transaction(PLACE_ORDER, |user| {
                Box::pin(async move {
                    let checkout = Checkout::builder().build();
                    let html =
                        woocommerce::place_order(user, &Shop::default(), CHECKOUT_BLOCK, &checkout)
                            .await?;
                    if !html.contains("order-confirmation-summary") {
                        return Err(wrong("not the order received page".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(PLACE_ORDER_ON_FILE, |user| {
                Box::pin(async move {
                    // The address the page has on file: nothing to type in.
                    let mut checkout = Checkout::builder();
                    for field in [
                        "first_name",
                        "last_name",
                        "address_1",
                        "city",
                        "postcode",
                        "email",
                        "phone",
                    ] {
                        checkout = checkout.billing(field, "");
                    }
                    let page = with_store_nonce(CHECKOUT_BLOCK, "onfile0");
                    woocommerce::place_order(user, &Shop::default(), &page, &checkout.build())
                        .await?;
                    Ok(())
                })
            }),
            transaction(PLACE_ORDER_EMPTY, |user| {
                Box::pin(async move {
                    expect_custom(
                        woocommerce::place_order(
                            user,
                            &Shop::default(),
                            CHECKOUT_BLOCK_EMPTY,
                            &Checkout::builder().build(),
                        )
                        .await,
                        "checkout/: the cart is empty",
                    )
                })
            }),
            transaction(PLACE_ORDER_NO_NONCE, |user| {
                Box::pin(async move {
                    let page = CHECKOUT_BLOCK.replace("storeApiNonce", "storeApiNonceGone");
                    expect_custom(
                        woocommerce::place_order(
                            user,
                            &Shop::default(),
                            &page,
                            &Checkout::builder().build(),
                        )
                        .await,
                        "checkout/: no Store API nonce on the page",
                    )
                })
            }),
            transaction(PLACE_ORDER_INVALID, |user| {
                Box::pin(async move {
                    let page = with_store_nonce(CHECKOUT_BLOCK, "invalid0");
                    woocommerce::place_order(
                        user,
                        &Shop::default(),
                        &page,
                        &Checkout::builder().build(),
                    )
                    .await?;
                    Ok(())
                })
            }),
            transaction(PLACE_ORDER_NO_NUMBER, |user| {
                Box::pin(async move {
                    let page = with_store_nonce(CHECKOUT_BLOCK, "nonumber0");
                    woocommerce::place_order(
                        user,
                        &Shop::default(),
                        &page,
                        &Checkout::builder().build(),
                    )
                    .await?;
                    Ok(())
                })
            }),
            transaction(CART_LOAD, |user| {
                Box::pin(async move {
                    let mut cart = cart("load/");
                    cart.load(user).await?;
                    if cart.nonce() != Some("n-load") || cart.cart_token() != Some("token-load") {
                        return Err(wrong(format!("{:?} {:?}", cart.nonce(), cart.cart_token())));
                    }
                    Ok(())
                })
            }),
            transaction(CART_ADD, |user| {
                Box::pin(async move {
                    let mut cart = cart("add/");
                    let json = cart.add_item(user, 31, 2).await?;
                    let item = &cart.items()[0];
                    if json["items"][0]["id"] != 31 || item.id != 31 || item.quantity != 2 {
                        return Err(wrong(format!("{:?}", cart.items())));
                    }
                    Ok(())
                })
            }),
            transaction(CART_NONCE_REFRESH, |user| {
                Box::pin(async move {
                    let mut cart = cart("refresh/");
                    cart.add_item(user, 31, 2).await?;
                    let key = cart.items()[0].key.clone();
                    cart.update_item(user, &key, 3).await?;
                    if cart.items()[0].quantity != 3 {
                        return Err(wrong(format!("{:?}", cart.items())));
                    }
                    cart.remove_item(user, &key).await?;
                    if !cart.items().is_empty() || cart.nonce() != Some("n-third") {
                        return Err(wrong(format!("{:?} {:?}", cart.items(), cart.nonce())));
                    }
                    Ok(())
                })
            }),
            transaction(CART_TOKEN, |user| {
                Box::pin(async move {
                    let mut cart = cart("token/").use_cart_token(true);
                    cart.add_item(user, 31, 2).await?;
                    Ok(())
                })
            }),
            transaction(CART_INVALID, |user| {
                Box::pin(async move {
                    cart("invalid/").add_item(user, 999999, 1).await?;
                    Ok(())
                })
            }),
            transaction(CART_OUT_OF_STOCK, |user| {
                Box::pin(async move {
                    cart("stock/").add_item(user, 30, 1).await?;
                    Ok(())
                })
            }),
            transaction(CART_MISSING_NONCE, |user| {
                Box::pin(async move {
                    cart("nononce/").add_item(user, 31, 1).await?;
                    Ok(())
                })
            }),
            transaction(CART_REMOVE_INVALID, |user| {
                Box::pin(async move {
                    cart("remove/").remove_item(user, "0000").await?;
                    Ok(())
                })
            }),
            transaction(CHECKOUT_ORDER, |user| {
                Box::pin(async move {
                    let mut cart = cart("checkout/");
                    let order = cart.checkout(user, &Checkout::builder().build()).await?;
                    if order["order_id"].as_u64().is_none() {
                        return Err(wrong(order.to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(CHECKOUT_CUSTOM, |user| {
                Box::pin(async move {
                    let checkout = Checkout::builder()
                        .billing("city", "Austin")
                        .billing("company", "Tag1")
                        .payment_method("cheque")
                        .build();
                    cart("custom/").checkout(user, &checkout).await?;
                    Ok(())
                })
            }),
            transaction(CHECKOUT_INVALID_ADDRESS, |user| {
                Box::pin(async move { check_out_at(user, "badaddress/").await })
            }),
            transaction(CHECKOUT_NOT_PAID, |user| {
                Box::pin(async move { check_out_at(user, "notpaid/").await })
            }),
            transaction(CHECKOUT_NO_ORDER, |user| {
                Box::pin(async move { check_out_at(user, "noorder/").await })
            }),
            transaction(CLASSIC_ADD_TO_CART, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().cart("classic-cart/").build();
                    woocommerce::classic_add_to_cart(user, &shop, 31, 2).await?;
                    Ok(())
                })
            }),
            transaction(CLASSIC_ADD_NOTHING, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().cart("classic-cart/").build();
                    woocommerce::classic_add_to_cart(user, &shop, 999999, 1).await?;
                    Ok(())
                })
            }),
            transaction(CLASSIC_UPDATE, |user| {
                Box::pin(async move { update_cart_at(user, "update/").await })
            }),
            transaction(CLASSIC_UPDATE_IGNORED, |user| {
                Box::pin(async move { update_cart_at(user, "ignored/").await })
            }),
            transaction(CLASSIC_UPDATE_NO_FORM, |user| {
                Box::pin(async move { update_cart_at(user, "noform/").await })
            }),
            transaction(CLASSIC_CHECKOUT_ORDER, |user| {
                Box::pin(async move {
                    let html = classic_check_out_at(user, "classic-checkout/").await?;
                    if !html.contains("woocommerce-order-received") {
                        return Err(wrong("not the order received page".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(CLASSIC_CHECKOUT_FAILED, |user| {
                Box::pin(async move {
                    classic_check_out_at(user, "failing-checkout/").await?;
                    Ok(())
                })
            }),
            transaction(CLASSIC_CHECKOUT_NO_FORM, |user| {
                Box::pin(async move {
                    classic_check_out_at(user, "noform/").await?;
                    Ok(())
                })
            }),
            transaction(MY_ACCOUNT_LOG_IN, |user| {
                Box::pin(async move { log_in_at(user, "my-account/", "secret").await })
            }),
            transaction(MY_ACCOUNT_WRONG, |user| {
                Box::pin(async move { log_in_at(user, "wrong-account/", "wrong").await })
            }),
            transaction(MY_ACCOUNT_NO_FORM, |user| {
                Box::pin(async move { log_in_at(user, "no-account/", "secret").await })
            }),
            transaction(MY_ACCOUNT_ORDERS, |user| {
                Box::pin(async move {
                    let html = woocommerce::my_account_orders(user, &Shop::default()).await?;
                    if !html.contains("woocommerce-MyAccount-navigation-link--orders is-active") {
                        return Err(wrong("not the orders".to_string()));
                    }
                    Ok(())
                })
            }),
            transaction(MY_ACCOUNT_ORDERS_LOGGED_OUT, |user| {
                Box::pin(async move {
                    let shop = Shop::builder().orders("logged-out/orders/").build();
                    woocommerce::my_account_orders(user, &shop).await?;
                    Ok(())
                })
            }),
        ],
    )
    .await;

    let mut problems: Vec<String> = vec![
        check_named(&metrics, SHOP_PAGE, &["shop/"], Expect::Pass),
        check_named(
            &metrics,
            SHOP_PAGE_WRONG,
            &["wrongshop/"],
            Expect::Fail("text not found on page: post-type-archive-product"),
        ),
        check_named(
            &metrics,
            SHOP_PAGE_VALIDATION,
            &["validshop/"],
            Expect::Fail("title not found: Not the shop"),
        ),
        check_named(&metrics, PRODUCT_PAGE, &[], Expect::Pass),
        check_named(
            &metrics,
            PRODUCT_PAGE_WRONG,
            &["product page"],
            Expect::Fail("text not found on page: single-product"),
        ),
        check_named(&metrics, PRODUCT_SEARCH_KEYS, &["product search"], Expect::Pass),
        check_named(&metrics, DISCOVER, &[], Expect::Pass),
        check_named(&metrics, DISCOVER_CAP, &[], Expect::Pass),
        check_named(&metrics, DISCOVER_STATUS, &["discover products"], Expect::Fail("500")),
        check_named(
            &metrics,
            DISCOVER_EMPTY,
            &["discover products"],
            Expect::Fail("emptyshop/: no product cards on the shop page"),
        ),
        check_named(&metrics, ADD_TO_CART, &[], Expect::Pass),
        check_named(
            &metrics,
            ADD_TO_CART_NO_FORM,
            &["product page"],
            Expect::Fail("/gone/cap/: no add to cart form on page: Out of stock"),
        ),
        check_named(
            &metrics,
            ADD_TO_CART_REFUSED,
            &["add to cart"],
            Expect::Fail("not added to cart: You cannot add"),
        ),
        check_named(
            &metrics,
            ADD_TO_CART_OPTIONS,
            &["product page"],
            Expect::Fail("needs a choice first"),
        ),
        check_named(&metrics, ARCHIVE_ADD, &[], Expect::Pass),
        check_named(
            &metrics,
            ARCHIVE_NO_BUTTON,
            &["product archive"],
            Expect::Fail("product 10 has no Add to cart button on the page"),
        ),
        check_named(
            &metrics,
            ARCHIVE_NO_NONCE,
            &["product archive"],
            Expect::Fail("the page's cart read gave no Store API nonce"),
        ),
        check_named(
            &metrics,
            ARCHIVE_REFUSED,
            &["wp-json/wc/store/v1/batch"],
            Expect::Fail("400 woocommerce_rest_product_out_of_stock"),
        ),
        check_named(&metrics, CART_PAGE, &[], Expect::Pass),
        check_named(&metrics, UPDATE_CART_ITEM, &[], Expect::Pass),
        check_named(&metrics, UPDATE_NOT_IN_CART, &[], Expect::Pass),
        check_named(&metrics, UPDATE_NO_NONCE, &[], Expect::Pass),
        check_named(
            &metrics,
            UPDATE_STALE_NONCE,
            &["wp-json/wc/store/v1/cart/update-item"],
            Expect::Fail("403"),
        ),
        check_named(&metrics, REMOVE_CART_ITEM, &[], Expect::Pass),
        check_named(&metrics, CHECKOUT_PAGE, &[], Expect::Pass),
        check_named(
            &metrics,
            CART_PAGE_WRONG,
            &["cart page"],
            Expect::Fail("text not found on page: woocommerce-cart"),
        ),
        check_named(
            &metrics,
            CHECKOUT_PAGE_WRONG,
            &["checkout page"],
            Expect::Fail("text not found on page: woocommerce-checkout"),
        ),
        check_named(&metrics, PLACE_ORDER, &[], Expect::Pass),
        check_named(&metrics, PLACE_ORDER_ON_FILE, &[], Expect::Pass),
        check_named(&metrics, PLACE_ORDER_EMPTY, &[], Expect::Pass),
        check_named(&metrics, PLACE_ORDER_NO_NONCE, &[], Expect::Pass),
        check_named(
            &metrics,
            PLACE_ORDER_INVALID,
            &["wp-json/wc/store/v1/checkout"],
            Expect::Fail("400"),
        ),
        check_named(
            &metrics,
            PLACE_ORDER_NO_NUMBER,
            &["order received"],
            Expect::Fail("no order number on the order received page"),
        ),
        check_named(&metrics, CART_LOAD, &["load/cart"], Expect::Pass),
        check_named(&metrics, CART_ADD, &["add/cart", "add/cart/add-item"], Expect::Pass),
        check_named(
            &metrics,
            CART_NONCE_REFRESH,
            &["refresh/cart/add-item", "refresh/cart/update-item", "refresh/cart/remove-item"],
            Expect::Pass,
        ),
        check_named(&metrics, CART_TOKEN, &["token/cart/add-item"], Expect::Pass),
        check_named(&metrics, CART_INVALID, &["invalid/cart/add-item"], Expect::Fail("400")),
        check_named(&metrics, CART_OUT_OF_STOCK, &["stock/cart/add-item"], Expect::Fail("400")),
        check_named(&metrics, CART_MISSING_NONCE, &["nononce/cart/add-item"], Expect::Fail("401")),
        check_named(&metrics, CART_REMOVE_INVALID, &["remove/cart/remove-item"], Expect::Fail("409")),
        check_named(&metrics, CHECKOUT_ORDER, &["checkout/checkout"], Expect::Pass),
        check_named(&metrics, CHECKOUT_CUSTOM, &["custom/checkout"], Expect::Pass),
        check_named(
            &metrics,
            CHECKOUT_INVALID_ADDRESS,
            &["badaddress/checkout"],
            Expect::Fail("400"),
        ),
        check_named(
            &metrics,
            CHECKOUT_NOT_PAID,
            &["notpaid/checkout"],
            Expect::Fail("notpaid/checkout: order not placed: payment failure"),
        ),
        check_named(
            &metrics,
            CHECKOUT_NO_ORDER,
            &["noorder/checkout"],
            Expect::Fail("noorder/checkout: order not placed: payment none"),
        ),
        check_named(&metrics, CLASSIC_ADD_TO_CART, &[], Expect::Pass),
        check_named(
            &metrics,
            CLASSIC_ADD_NOTHING,
            &["classic add to cart"],
            Expect::Fail("not added to cart: no success notice"),
        ),
        check_named(&metrics, CLASSIC_UPDATE, &["update/"], Expect::Pass),
        check_named(
            &metrics,
            CLASSIC_UPDATE_IGNORED,
            &["ignored/"],
            Expect::Fail("cart not updated: no success notice"),
        ),
        check_named(
            &metrics,
            CLASSIC_UPDATE_NO_FORM,
            &["noform/"],
            Expect::Fail("noform/: no cart form with a nonce on page"),
        ),
        check_named(
            &metrics,
            CLASSIC_CHECKOUT_ORDER,
            // Not `order received`: place_order's failing case shares the name.
            &["classic-checkout/"],
            Expect::Pass,
        ),
        check_named(
            &metrics,
            CLASSIC_CHECKOUT_FAILED,
            &["?wc-ajax=checkout"],
            Expect::Fail("checkout failed: The following problems were found: Billing Last name is a required field."),
        ),
        check_named(
            &metrics,
            CLASSIC_CHECKOUT_NO_FORM,
            &["noform/"],
            Expect::Fail("noform/: no checkout form with a nonce on page"),
        ),
        check_named(&metrics, MY_ACCOUNT_LOG_IN, &["my-account/"], Expect::Pass),
        check_named(
            &metrics,
            MY_ACCOUNT_WRONG,
            &["wrong-account/"],
            Expect::Fail("wrong-account/: log in failed: Error: The password you entered for the username customer is incorrect."),
        ),
        check_named(
            &metrics,
            MY_ACCOUNT_NO_FORM,
            &["no-account/"],
            Expect::Fail("no-account/: no log in form with a nonce on page"),
        ),
        check_named(&metrics, MY_ACCOUNT_ORDERS, &["my-account/orders/"], Expect::Pass),
        check_named(
            &metrics,
            MY_ACCOUNT_ORDERS_LOGGED_OUT,
            &["logged-out/orders/"],
            Expect::Fail("text not found on page: woocommerce-MyAccount-navigation"),
        ),
    ]
    .into_iter()
    .flatten()
    .collect();

    // Each mock that proves a request was made its expected number of times.
    let counts = [
        (PRODUCT_SEARCH_KEYS, "searched", search.hits(), 1),
        (
            DISCOVER,
            "second shop page read, not with the cap",
            shop_page_2.hits(),
            1,
        ),
        (ADD_TO_CART, "form posted", product_add.hits(), 1),
        (ARCHIVE_ADD, "added from the archive", archive_add.hits(), 1),
        (
            ARCHIVE_ADD,
            "Store API cart read by each archive",
            archive_read.hits(),
            3,
        ),
        (UPDATE_CART_ITEM, "updated", update_item.hits(), 1),
        (REMOVE_CART_ITEM, "removed", remove_item.hits(), 1),
        (
            PLACE_ORDER,
            "email, then address, sent",
            customer_updates.hits(),
            2,
        ),
        (PLACE_ORDER, "ordered", place.hits(), 1),
        (
            PLACE_ORDER,
            "order received loaded (and on file)",
            block_received.hits(),
            2,
        ),
        (
            PLACE_ORDER_ON_FILE,
            "ordered without updates",
            on_file.hits(),
            1,
        ),
        (CART_LOAD, "cart loaded", load.hits(), 1),
        (CART_ADD, "added with the nonce", add.hits(), 1),
        (
            CART_NONCE_REFRESH,
            "updated with the second nonce",
            refreshed_update.hits(),
            1,
        ),
        (
            CART_NONCE_REFRESH,
            "removed with the third nonce",
            refreshed_remove.hits(),
            1,
        ),
        (CART_TOKEN, "added with the cart token", token.hits(), 1),
        (CHECKOUT_ORDER, "checked out", checkouts[0].hits(), 1),
        (CHECKOUT_CUSTOM, "checked out", checkouts[1].hits(), 1),
        (CLASSIC_ADD_TO_CART, "added", classic_add.hits(), 1),
        (
            CLASSIC_CHECKOUT_ORDER,
            "checked out",
            classic_checkout.hits(),
            1,
        ),
        (
            CLASSIC_CHECKOUT_ORDER,
            "order received loaded",
            order_received.hits(),
            1,
        ),
    ];
    for (behaviour, what, hits, expected) in counts {
        if hits != expected {
            problems.push(format!(
                "behaviour {}: {} {} times, expected {}",
                behaviour, what, hits, expected
            ));
        }
    }
    common::assert_all(problems);
}

// A product as its card on the shop page shows it.
fn product(id: u64, url: &str) -> Product {
    Product {
        id,
        name: format!("Product {}", id),
        url: url.to_string(),
        kind: "simple".to_string(),
        purchasable: true,
        in_stock: true,
        has_options: false,
    }
}

// A response header saved with a fixture.
fn header_of(headers: &str, name: &str) -> String {
    headers
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{}: ", name)))
        .unwrap_or_else(|| panic!("no {} header saved", name))
        .to_string()
}

// The Store API nonce a block cart or checkout page carries.
fn store_nonce(html: &str) -> String {
    regex::Regex::new(r"storeApiNonce: '([^']+)'")
        .unwrap()
        .captures(html)
        .unwrap()[1]
        .to_string()
}

// A block cart or checkout page carrying `nonce` as its Store API nonce.
fn with_store_nonce(html: &str, nonce: &str) -> String {
    html.replace(&store_nonce(html), nonce)
}

// Pass when `result` failed the transaction with a reason holding `text`, as
// the cart and checkout helpers do when a page cannot do what was asked: no
// request was made, so the reason is the transaction's, not a request's.
fn expect_custom<T>(result: Result<T, Box<TransactionError>>, text: &str) -> TransactionResult {
    match result {
        Err(e) => match e.as_ref() {
            TransactionError::Custom(reason) if reason.contains(text) => Ok(()),
            other => Err(wrong(format!("failed with {}, expected {:?}", other, text))),
        },
        Ok(_) => Err(wrong(format!("passed, expected a failure with {:?}", text))),
    }
}

// A cart on the Store API root `root`.
fn cart(root: &str) -> StoreApiCart {
    StoreApiCart::new(&Shop::builder().store_api(root).build())
}

// Check out a cart on `root` with the default details.
async fn check_out_at(user: &mut GooseUser, root: &str) -> TransactionResult {
    cart(root)
        .checkout(user, &Checkout::builder().build())
        .await?;
    Ok(())
}

// Empty the classic cart at `/{path}`.
async fn update_cart_at(user: &mut GooseUser, path: &str) -> TransactionResult {
    let shop = Shop::builder().cart(path).build();
    woocommerce::classic_update_cart(user, &shop, 0).await?;
    Ok(())
}

// Check out the classic checkout at `/{path}` with the default details.
async fn classic_check_out_at(
    user: &mut GooseUser,
    path: &str,
) -> Result<String, Box<TransactionError>> {
    let shop = Shop::builder().checkout(path).build();
    woocommerce::classic_checkout(user, &shop, &Checkout::builder().build()).await
}

// Log into My Account at `/{path}` as the customer.
async fn log_in_at(user: &mut GooseUser, path: &str, password: &str) -> TransactionResult {
    let shop = Shop::builder().my_account(path).build();
    woocommerce::my_account_log_in(user, &shop, "customer", password).await?;
    Ok(())
}
