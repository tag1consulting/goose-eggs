# Goose Eggs

Helpful in writing [`Goose`](https://docs.rs/goose) load tests.

[![crates.io](https://img.shields.io/crates/v/goose-eggs.svg)](https://crates.io/crates/goose-eggs)
[![Documentation](https://docs.rs/goose-eggs/badge.svg)](https://docs.rs/goose-eggs)
[![Apache-2.0 licensed](https://img.shields.io/crates/l/goose-eggs.svg)](./LICENSE)
[![CI](https://github.com/tag1consulting/goose-eggs/workflows/CI/badge.svg)](https://github.com/tag1consulting/goose-eggs/actions?query=workflow%3ACI)

## Goose compatibility

Each goose-eggs minor release works with one Goose minor release:

| goose-eggs | Goose |
|---|---|
| 0.7 | 0.19 |
| 0.6 | 0.18 |
| 0.5 | 0.17 |

goose-eggs 0.7 is on crates.io for Goose 0.19; between releases, the `main` branch follows Goose `main`, see [RELEASING.md](RELEASING.md).

## CMS helpers

Besides page validation and static asset loading, goose-eggs has modules for the forms and APIs of three platforms. Each helper takes `Validate` rules for the pages it loads, fails the transaction with the platform's own reason, and has every path configurable on its builder.

### Drupal

`goose_eggs::drupal` logs in (`Login`, `log_in`), searches (`SearchParams`, `search`), and reads Drupal forms, including ones BigPipe streams (`get_form`, `get_form_value`, `get_bigpipe_form`, `get_encoded_form_value`, `get_updated_build_id`). The [Umami example](examples/umami) is a complete load test of Drupal's Umami demo.

### WordPress

`goose_eggs::wordpress` drives any WordPress site through what core serves:

- `Login` and `log_in`, `log_out`: the log in form with WordPress's test cookie, and the nonce in the log out link
- `SearchParams` and `search`: `?s=`, optionally limited to a post type
- `Comment` and `post_comment`: the comment form, as an anonymous or logged in user, with random text by default so WordPress's duplicate check passes
- `Post`, `create_post` and `update_post`: the block editor page with its assets, then the REST save it sends with the page's nonce; `PostBuilder::post_type` names a custom post type's slug for its editor; `PostBuilder::rest_nonce` sends only the REST request, with a nonce from `get_rest_nonce`
- `Discover`, `discover` and `SiteContent`: the site's real URLs by kind, from the core sitemap or the REST API, to request at random
- `get_form`, `get_form_value`, `get_form_values` and `get_nonce`: form parsing that reads attributes in any order, as WordPress writes them

`examples/wordpress-sites/theme-test-data` brings up WordPress with the theme unit test data in Docker; [the WooCommerce example's README](examples/woocommerce/README.md#the-wordpress-helpers) shows the helpers driving it.

### WooCommerce

`goose_eggs::woocommerce`, built on `wordpress`, drives any WooCommerce shop:

- `shop_page`, `product_page`, `product_search`, and `discover_products` with `random_product`, through the Store API
- `StoreApiCart`: the cart and checkout WooCommerce's block cart and checkout use, keeping each user's Store API nonce, with a `Checkout` builder (Cash on delivery by default)
- `classic_add_to_cart`, `classic_update_cart` and `classic_checkout`: the shortcode cart and checkout
- `my_account_log_in` and `my_account_orders`
- `Shop`: where the shop's pages and Store API are, defaulting to WooCommerce's

The [WooCommerce example](examples/woocommerce) is a complete load test of a shop, and `examples/wordpress-sites/woocommerce` brings up the demo shop it runs against in Docker.

## Compile regular expressions once

A load test runs each transaction thousands of times, so a `Regex::new` in a transaction or a helper it calls compiles its pattern and rebuilds its match cache on every request; in Goose's Umami example that cost about half of Goose's CPU per request. Compile each pattern once, in a `static` with [`std::sync::LazyLock`](https://doc.rust-lang.org/std/sync/struct.LazyLock.html), and share it across every user and request:

```rust
use regex::Regex;
use std::sync::LazyLock;

// Compiled the first time any user needs it, then reused.
static ORDER_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Order number: <strong>(\d+)</strong>").unwrap());

fn order_number(html: &str) -> Option<String> {
    ORDER_NUMBER.captures(html).map(|c| c[1].to_string())
}

let page = "<li>Order number: <strong>42</strong></li>";
assert_eq!(order_number(page), Some("42".to_string()));
```

## Documentation

- [Goose Eggs](https://docs.rs/goose-eggs)
- [Goose Eggs Umami example](https://github.com/tag1consulting/goose-eggs/tree/main/examples/umami)
- [Goose Eggs WooCommerce example](https://github.com/tag1consulting/goose-eggs/tree/main/examples/woocommerce)
- [The Goose book](https://book.goose.rs/)
