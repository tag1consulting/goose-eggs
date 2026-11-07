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

`goose_eggs::wordpress` drives any WordPress site the way a visitor's or an editor's browser does: each helper loads the pages a browser loads, with the static assets their markup names, and sends only the background requests those pages' own scripts send, as recorded from headless Chromium in [`tests/fixtures/browser`](tests/fixtures/browser/README.md):

- `view_page`: any page, with its assets and background requests

- `Login` and `log_in`, `log_out`: the log in form with WordPress's test cookie, and the nonce in the log out link
- `SearchParams` and `search`: `?s=`, optionally limited to a post type
- `Comment` and `post_comment`: the comment form, as an anonymous or logged in user, with random text by default so WordPress's duplicate check passes
- `Post`, `create_post` and `update_post`: the block editor, opened with its assets and the reads its scripts make, then saved as its buttons save, with the nonce from the page
- `Discover`, `discover` and `SiteContent`: the site's real URLs by kind, found by browsing its navigation, listings and pagination, to request at random
- `get_form`, `get_form_value`, `get_form_values` and `get_nonce`: form parsing that reads attributes in any order, as WordPress writes them

`examples/wordpress-sites/theme-test-data` brings up WordPress with the theme unit test data in Docker; [the WooCommerce example's README](examples/woocommerce/README.md#the-wordpress-helpers) shows the helpers driving it.

### WooCommerce

`goose_eggs::woocommerce`, built on `wordpress`, drives any WooCommerce shop the same way:

- `shop_page`, `product_page`, `product_search`, and `discover_products` with `random_product`, reading the shop page's product cards
- `add_to_cart` from a product page, `add_to_cart_from_archive` from the shop page's buttons
- `cart_page`, `update_cart_item` and `remove_cart_item`: the block cart and its controls
- `checkout_page` and `place_order`: the block checkout and its order, with a `Checkout` builder (Cash on delivery by default)
- `StoreApiCart`: the Store API driven directly, as a headless storefront does, for load testing one on purpose; not what a browser on a standard theme does
- `classic_add_to_cart`, `classic_update_cart` and `classic_checkout`: the shortcode cart and checkout
- `my_account_log_in` and `my_account_orders`
- `Shop`: where the shop's pages and Store API are, defaulting to WooCommerce's

The `wordpress` and `woocommerce` CI jobs run a parity test, `tests/sites/parity.rs`, which compares every helper's requests against the browser's recording of the same action.

The [WooCommerce example](examples/woocommerce) is a complete load test of a shop, and `examples/wordpress-sites/woocommerce` brings up the demo shop it runs against in Docker.

## Documentation

- [Goose Eggs](https://docs.rs/goose-eggs)
- [Goose Eggs Umami example](https://github.com/tag1consulting/goose-eggs/tree/main/examples/umami)
- [Goose Eggs WooCommerce example](https://github.com/tag1consulting/goose-eggs/tree/main/examples/woocommerce)
- [The Goose book](https://book.goose.rs/)
