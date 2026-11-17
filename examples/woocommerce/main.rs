mod common;
mod customer;

use goose::prelude::*;
use goose_eggs::woocommerce::{self, StoreApiCart};
use std::time::Duration;

use crate::common::*;

/// Defines the load test. Each scenario simulates a kind of shopper:
///  - Browsing shopper: the shop page, category archives, product pages and search
///  - Buyer: browses products, puts one to three in the cart, views it, and
///    sometimes checks out with Cash on delivery
///  - Returning customer: logs into My Account with its own account, checks
///    past orders, and buys
#[tokio::main]
async fn main() -> Result<(), GooseError> {
    let _goose_metrics = GooseAttack::initialize()?
        .register_scenario(
            scenario!("Browsing shopper")
                .set_weight(6)?
                .set_wait_time(Duration::from_secs(0), Duration::from_secs(3))?
                .register_transaction(
                    transaction!(discover_catalog)
                        .set_on_start()
                        .set_name("discover catalog"),
                )
                .register_transaction(transaction!(shop_page).set_name("shop"))
                .register_transaction(
                    transaction!(category_page)
                        .set_name("category")
                        .set_weight(2)?,
                )
                .register_transaction(
                    transaction!(product_page)
                        .set_name("product")
                        .set_weight(4)?,
                )
                .register_transaction(transaction!(product_search).set_name("search")),
        )
        .register_scenario(
            scenario!("Buyer")
                .set_weight(3)?
                .set_wait_time(Duration::from_secs(0), Duration::from_secs(3))?
                .register_transaction(
                    transaction!(discover_catalog)
                        .set_on_start()
                        .set_name("discover catalog"),
                )
                .register_transaction(
                    transaction!(product_page)
                        .set_name("product")
                        .set_weight(3)?,
                )
                .register_transaction(
                    transaction!(fill_cart)
                        .set_name("add to cart")
                        .set_weight(2)?,
                )
                .register_transaction(transaction!(view_cart).set_name("view cart"))
                .register_transaction(transaction!(check_out).set_name("checkout")),
        )
        .register_scenario(
            scenario!("Returning customer")
                .set_weight(1)?
                .set_wait_time(Duration::from_secs(0), Duration::from_secs(3))?
                .register_transaction(
                    transaction!(discover_catalog)
                        .set_on_start()
                        .set_name("discover catalog"),
                )
                .register_transaction(
                    transaction!(log_in)
                        .set_on_start()
                        .set_name("my account log in"),
                )
                .register_transaction(transaction!(orders).set_name("my account orders"))
                .register_transaction(
                    transaction!(product_page)
                        .set_name("product")
                        .set_weight(2)?,
                )
                .register_transaction(transaction!(fill_cart).set_name("add to cart"))
                .register_transaction(transaction!(check_out).set_name("checkout")),
        )
        .set_default(GooseDefault::Host, "http://localhost:18082/")?
        .execute()
        .await?;

    Ok(())
}

/// Load the shop page.
async fn shop_page(user: &mut GooseUser) -> TransactionResult {
    view(user, "shop/", "shop", "post-type-archive-product").await
}

/// Load a random product category archive.
async fn category_page(user: &mut GooseUser) -> TransactionResult {
    match catalog().content.random("product_cat") {
        Some(category) => view(user, category, "product category", "tax-product_cat").await,
        None => Ok(()),
    }
}

/// Load a random product's page.
async fn product_page(user: &mut GooseUser) -> TransactionResult {
    let product = random_product();
    view(user, &product.url, "product page", "single-product").await
}

/// Search for a word from a product's name. WooCommerce shows the results, or
/// the product itself when only one matches.
async fn product_search(user: &mut GooseUser) -> TransactionResult {
    let path = format!("?s={}&post_type=product", random_search_word());
    view(user, &path, "product search", "woocommerce-page").await
}

/// Load the cart page.
async fn view_cart(user: &mut GooseUser) -> TransactionResult {
    view(user, "cart/", "cart", "wp-block-woocommerce-cart").await
}

/// Log in as this user's customer. The cart's nonce was the visitor's, so the
/// cart starts again and gets the customer's on its first write.
async fn log_in(user: &mut GooseUser) -> TransactionResult {
    let (username, password) = customer(user);
    woocommerce::my_account_log_in(user, &shop(), &username, &password).await?;
    user.set_session_data(StoreApiCart::new(&shop()));
    Ok(())
}

/// Load the customer's orders.
async fn orders(user: &mut GooseUser) -> TransactionResult {
    view(
        user,
        "my-account/orders/",
        "my account orders",
        "woocommerce-MyAccount-navigation",
    )
    .await
}
