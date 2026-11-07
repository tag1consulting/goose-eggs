# Overview

A load test of a WooCommerce shop written with Goose Eggs' `woocommerce` and `wordpress` helpers. It runs against any WooCommerce shop on WooCommerce's default pages and the default block cart and checkout, and makes the requests a shopper's browser makes: the pages, their static assets, and only the background requests those pages' own scripts send. It learns the shop's products from the product cards on its shop pages, and its category archives by browsing the shop from its home page, so nothing about the catalog is hard coded. It does that once, before the load test starts, as its `test_start` transaction: Goose runs it on a user of its own before any user starts, so the crawl is in neither the metrics nor the request log, and every user starts with the catalog in hand. If the crawl fails, every user fails its first transaction (`catalog`) with the reason and the load test stops.

It has three kinds of users:

- **Browsing shopper** (weight 6): the shop page, product category archives, product pages, and product searches for words from product names.
- **Buyer** (weight 3): browses products, puts one to three products in the cart from their pages' add to cart forms, views the cart page, and goes to the checkout page and places the order with Cash on delivery. The checkout transaction has the lowest weight, so most carts are abandoned, as in a real shop.
- **Returning customer** (weight 1): logs into My Account with its own customer account when it starts, then views past orders, browses, and buys, checking out with the address its account has on file.

## Running it locally

`examples/wordpress-sites/woocommerce/setup.sh` starts the demo shop in Docker, the same one the `woocommerce` CI job runs against: the official `wordpress` and `mariadb` images, WordPress installed with `wp-cli`, WooCommerce with its bundled sample products, its own cart, checkout and My Account pages, Cash on delivery, free shipping everywhere, and 100 customer accounts. Versions are pinned in the compose file and the script. From the repository root:

```sh
examples/wordpress-sites/woocommerce/setup.sh start
cargo run --example woocommerce -- --host http://localhost:18082 --users 5 --increase-rate 5 --run-time 60s
examples/wordpress-sites/woocommerce/setup.sh stop
```

`scripts/woocommerce.sh` does what the CI job does: it starts the shop, runs the example at 30 users for 60 seconds (three of them returning customers, so their logins and carts run side by side), fails on any request or transaction error, and checks with `wp wc shop_order list` that the run placed orders. Set `WOOCOMMERCE_SITE_RUNNING=1` to reuse a shop that is already up. `examples/wordpress-sites/woocommerce/setup.sh wp ...` runs any other `wp-cli` command against the shop.

The shop listens on port 18082; set `PORT` for `setup.sh` (or `WOOCOMMERCE_PORT` for `scripts/woocommerce.sh`) to use another port, and pass the same port in `--host`.

## Customer credentials

Each returning customer logs in to an account of its own: Goose's user 1 (the user with `weighted_users_index` 0) as `customer1`, user 8 as `customer8`, and so on, all with the password `customer-password`. The scenario weights make every tenth user a returning customer, so at 100 users they log in as `customer8`, `customer18` and so on to `customer98`.

They need an account each because WordPress and WooCommerce keep a logged in customer's cart and login sessions with the account, not with the browser. Returning customers sharing one account share one cart, so one checking out empties the cart another is about to check out (`409 Conflict`, `woocommerce_rest_cart_empty`, on `wp-json/wc/store/v1/checkout`). They also overwrite each other's login sessions when they log in at the same moment, so a login can succeed and the next page come back logged out (`text not found on page: woocommerce-MyAccount-navigation` on `my-account/orders/`).

`setup.sh` creates `customer1` to `customer100`; set `CUSTOMER_COUNT` to create more, one for every user of the largest run, as a user whose account does not exist fails to log in. `CUSTOMER_PASSWORD` sets the password of all of them, for both `setup.sh` and the example.

Set `CUSTOMER_USERNAME` to log every returning customer in to that one account instead, for a shop with a single customer account. That runs clean only while at most one returning customer runs at a time (fewer than 18 users). `setup.sh` also creates that account, `customer` by default, with the same password. The shop's admin is `admin` / `admin-password` (`ADMIN_USERNAME`, `ADMIN_PASSWORD`).

## Logging in in the metrics

Logging in is an on start transaction (`my account log in`), run once by each returning customer as it starts. Goose resets its metrics once every user has started, unless it runs with `--no-reset-metrics`, so the final tables and report show only the logins that finished after the last user started: few or none. Run with `--no-reset-metrics`, as `scripts/woocommerce.sh` does, to see one login counted for each returning customer, 10 at 100 users.

## Another shop

Point `--host` at any WooCommerce shop with Cash on delivery enabled and customer accounts `customer1` to `customerN` for N users, or one account named in `CUSTOMER_USERNAME`. A shop whose pages are not at WooCommerce's defaults needs `common::shop()` changed to a `Shop::builder()` naming them, for example a shop on the classic shortcode cart and checkout:

```rust
use goose_eggs::woocommerce::Shop;

let shop = Shop::builder().cart("basket/").checkout("pay/").build();
```

The classic flows (`classic_add_to_cart`, `classic_update_cart`, `classic_checkout`) drive the shortcode cart and checkout; the demo shop has them at `classic-cart/` and `classic-checkout/`.

## The wordpress helpers

`goose_eggs::wordpress` drives any WordPress site the same way. `examples/wordpress-sites/theme-test-data/setup.sh start` brings up WordPress with the theme unit test data (posts, pages, categories, tags and comments of every shape) on port 18081, with an `editor` account (`editor-password`). A load test of it reads the site's real URLs once, before it starts, then requests them at random, comments, searches and edits:

```rust
use goose::prelude::*;
use goose_eggs::wordpress::{self, Comment, Discover, Login, Post, SearchParams, SiteContent};
use std::sync::OnceLock;

static CONTENT: OnceLock<Result<SiteContent, String>> = OnceLock::new();

// Registered with `GooseAttack::test_start(transaction!(find_content))`: Goose runs
// it once, before any user starts, and does not measure it.
async fn find_content(user: &mut GooseUser) -> TransactionResult {
    let found = wordpress::discover(user, &Discover::builder().build()).await;
    let _ = CONTENT.set(found.map_err(|error| error.to_string()));
    Ok(())
}

// Each user's on start transaction.
async fn start(user: &mut GooseUser) -> TransactionResult {
    let login = Login::builder().username("editor").password("editor-password").build();
    wordpress::log_in(user, &login).await?;
    Ok(())
}

async fn read_and_comment(user: &mut GooseUser) -> TransactionResult {
    let post = match CONTENT.get() {
        Some(Ok(content)) => content.random("post").unwrap_or("/"),
        Some(Err(error)) => return Err(Box::new(TransactionError::Custom(error.clone()))),
        None => return Err(Box::new(TransactionError::Custom("no content".to_string()))),
    };
    let comment = Comment::builder().post_url(post).build();
    wordpress::post_comment(user, &comment).await?;
    Ok(())
}

async fn search(user: &mut GooseUser) -> TransactionResult {
    wordpress::search(user, &SearchParams::builder().keys("post").build()).await?;
    Ok(())
}

async fn edit(user: &mut GooseUser) -> TransactionResult {
    // Open the block editor on a new post and save the draft, then open it again
    // and update it, as an editor does; each takes its nonce from the editor page.
    let draft = wordpress::create_post(user, &Post::builder().title("Draft").build()).await?;
    let id = draft["id"].as_u64().unwrap_or_default();
    wordpress::update_post(user, id, &Post::builder().title("Edited").build()).await?;
    Ok(())
}
```

WordPress serves the block editor without the classic editor's TinyMCE to a `User-Agent` it does not know, such as Goose's; to load what a browser loads there, give Goose a browser's with `GooseAttack::set_client_builder_with_cookies(GooseClientBuilder::new().user_agent(...))`, as `tests/sites/parity.rs` does. Some posts in the test data have comments closed, so a load test commenting on random posts sees `post_comment` fail on those; pick posts known to take comments, or count those failures as expected. WordPress also throttles comments from one address (a 429 within 15 seconds of the last one), which every anonymous Goose user shares; logged in editors are not throttled. `scripts/wordpress.sh`, the `wordpress` CI job, runs `tests/sites/wordpress.rs`, which drives every one of these helpers against the site, and `tests/sites/parity.rs`, which compares their requests with a browser's.
