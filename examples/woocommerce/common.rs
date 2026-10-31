use goose::prelude::*;
use goose_eggs::woocommerce::{self, Checkout, Product, Shop, StoreApiCart};
use goose_eggs::wordpress::{self, SiteContent};
use rand::prelude::{IndexedRandom, IteratorRandom};
use rand::Rng;
use std::env;
use std::sync::OnceLock;

/// What the shop sells and how its categories are reached, found once by the
/// first user to start and shared by every user after.
pub struct Catalog {
    pub products: Vec<Product>,
    pub content: SiteContent,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

/// The shop, on WooCommerce's default paths.
pub fn shop() -> Shop<'static> {
    Shop::default()
}

/// Find the shop's products through the Store API, and its category archives
/// through the core sitemap, unless another user already has.
pub async fn discover_catalog(user: &mut GooseUser) -> TransactionResult {
    if CATALOG.get().is_none() {
        let products = woocommerce::discover_products(user, &shop(), 500).await?;
        let content = wordpress::discover(user, &wordpress::Discover::builder().build()).await?;
        // Another user may have finished first; either catalog will do.
        let _ = CATALOG.set(Catalog { products, content });
    }
    Ok(())
}

/// The shared catalog; discovery runs on start, before any of these is needed.
pub fn catalog() -> &'static Catalog {
    CATALOG.get().expect("the catalog is discovered on start")
}

/// A random product that can go in the cart.
pub fn random_product() -> &'static Product {
    woocommerce::random_product(&catalog().products).expect("the shop sells a simple product")
}

/// A word from a random product's name, to search for.
pub fn random_search_word() -> String {
    let product = catalog()
        .products
        .choose(&mut rand::rng())
        .expect("the shop has products");
    product
        .name
        .split_whitespace()
        .choose(&mut rand::rng())
        .unwrap_or("shirt")
        .to_lowercase()
}

/// This user's username and password: its own demo shop customer, unless
/// `CUSTOMER_USERNAME` names one account for every user, with the password from
/// `CUSTOMER_PASSWORD` (see `crate::customer::credentials`).
pub fn customer(user: &GooseUser) -> (String, String) {
    crate::customer::credentials(
        user.weighted_users_index,
        env::var("CUSTOMER_USERNAME").ok(),
        env::var("CUSTOMER_PASSWORD").ok(),
    )
}

/// This user's Store API cart, taken out of its session data (put it back with
/// `user.set_session_data`).
pub fn take_cart(user: &mut GooseUser) -> StoreApiCart {
    user.get_session_data::<StoreApiCart>()
        .cloned()
        .unwrap_or_else(|| StoreApiCart::new(&shop()))
}

/// Add one to three random products to this user's cart.
pub async fn fill_cart(user: &mut GooseUser) -> TransactionResult {
    let mut cart = take_cart(user);
    let lines: usize = rand::rng().random_range(1..=3);
    for _ in 0..lines {
        let product = random_product();
        let quantity = rand::rng().random_range(1..=2);
        cart.add_item(user, product.id, quantity).await?;
    }
    user.set_session_data(cart);
    Ok(())
}

/// Check out this user's cart with Cash on delivery, filling it first if empty.
pub async fn check_out(user: &mut GooseUser) -> TransactionResult {
    if take_cart(user).items().is_empty() {
        fill_cart(user).await?;
    }
    let mut cart = take_cart(user);
    let result = cart.checkout(user, &Checkout::builder().build()).await;
    user.set_session_data(cart);
    result.map(|_| ())
}
