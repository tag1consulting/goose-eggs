use goose::prelude::*;
use goose_eggs::{woocommerce, wordpress};
use std::time::Duration;

use crate::common::*;

/// Registers the load test's scenarios on `attack`, so `main` and the tests run
/// the same load test. The catalog is found once, before any user starts
/// (`discover_catalog`, run as `test_start`); every user then checks it is there
/// (`catalog_ready`) and reads it. Each scenario simulates a kind of shopper:
///  - Browsing shopper: the shop page, category archives, product pages and search
///  - Buyer: browses products, puts one to three in the cart from their pages,
///    views the cart, and sometimes goes to checkout and places the order with
///    Cash on delivery
///  - Returning customer: logs into My Account with its own account, checks
///    past orders, and buys
pub fn load_test(attack: GooseAttack) -> Result<Box<GooseAttack>, GooseError> {
    attack
        .test_start(transaction!(discover_catalog).set_name("discover catalog"))
        .register_scenario(
            scenario!("Browsing shopper")
                .set_weight(6)?
                .set_wait_time(Duration::from_secs(0), Duration::from_secs(3))?
                .register_transaction(
                    transaction!(catalog_ready)
                        .set_on_start()
                        .set_name("catalog"),
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
                    transaction!(catalog_ready)
                        .set_on_start()
                        .set_name("catalog"),
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
                .register_transaction(transaction!(buyer_check_out).set_name("checkout")),
        )
        .register_scenario(
            scenario!("Returning customer")
                .set_weight(1)?
                .set_wait_time(Duration::from_secs(0), Duration::from_secs(3))?
                .register_transaction(
                    transaction!(catalog_ready)
                        .set_on_start()
                        .set_name("catalog"),
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
                .register_transaction(transaction!(customer_check_out).set_name("checkout")),
        )
        .set_default(GooseDefault::Host, "http://localhost:18082/")
}

/// Load the shop page.
async fn shop_page(user: &mut GooseUser) -> TransactionResult {
    catalog()?;
    woocommerce::shop_page(user, &shop(), None).await?;
    Ok(())
}

/// Load a random product category archive.
async fn category_page(user: &mut GooseUser) -> TransactionResult {
    if let Some(category) = catalog()?.content.random("product_cat") {
        let validate = goose_eggs::Validate::builder()
            .text("tax-product_cat")
            .build();
        wordpress::view_page(user, category, "product category", Some(&validate)).await?;
    }
    Ok(())
}

/// Load a random product's page.
async fn product_page(user: &mut GooseUser) -> TransactionResult {
    let product = random_product()?;
    woocommerce::product_page(user, &product.url, None).await?;
    Ok(())
}

/// Search for a word from a product's name.
async fn product_search(user: &mut GooseUser) -> TransactionResult {
    woocommerce::product_search(user, &random_search_word()?, None).await?;
    Ok(())
}

/// View the cart page.
async fn view_cart(user: &mut GooseUser) -> TransactionResult {
    catalog()?;
    woocommerce::cart_page(user, &shop()).await?;
    Ok(())
}

/// Check out as a new shopper, typing in an address.
async fn buyer_check_out(user: &mut GooseUser) -> TransactionResult {
    check_out(user, false).await
}

/// Check out as a returning customer, with the address on file.
async fn customer_check_out(user: &mut GooseUser) -> TransactionResult {
    check_out(user, true).await
}

/// Log in as this user's customer on My Account.
async fn log_in(user: &mut GooseUser) -> TransactionResult {
    catalog()?;
    let (username, password) = customer(user);
    woocommerce::my_account_log_in(user, &shop(), &username, &password).await?;
    Ok(())
}

/// Load the customer's orders.
async fn orders(user: &mut GooseUser) -> TransactionResult {
    catalog()?;
    woocommerce::my_account_orders(user, &shop()).await?;
    Ok(())
}
