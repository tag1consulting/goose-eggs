use goose::prelude::*;
use goose_eggs::woocommerce::{self, Checkout, Product, Shop};
use goose_eggs::wordpress::{self, SiteContent};
use rand::prelude::{IndexedRandom, IteratorRandom};
use rand::Rng;
use std::env;
use tokio::sync::OnceCell;

/// What the shop sells and how its categories are reached, found once by the
/// first user to start and shared by every user after.
pub struct Catalog {
    pub products: Vec<Product>,
    pub content: SiteContent,
}

static CATALOG: OnceCell<Catalog> = OnceCell::const_new();

/// The shop, on WooCommerce's default paths.
pub fn shop() -> Shop<'static> {
    Shop::default()
}

/// Find the shop's products from the cards on its shop pages, and its category
/// archives by browsing it from the home page, as a visitor would. The first
/// user to start does it; every other user waits for that one.
pub async fn discover_catalog(user: &mut GooseUser) -> TransactionResult {
    CATALOG
        .get_or_try_init(|| async {
            let products = woocommerce::discover_products(user, &shop(), 500).await?;
            let discover = wordpress::Discover::builder().max_per_kind(50).build();
            let content = wordpress::discover(user, &discover).await?;
            Ok::<_, Box<TransactionError>>(Catalog { products, content })
        })
        .await?;
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

/// How many products this user has put in its cart since it last checked out,
/// kept in its session data: the cart itself lives in the shop.
#[derive(Clone, Copy, Debug, Default)]
pub struct Basket {
    pub lines: usize,
}

fn basket(user: &GooseUser) -> Basket {
    user.get_session_data::<Basket>()
        .copied()
        .unwrap_or_default()
}

/// Add one to three random products to this user's cart, each from its product
/// page, as a shopper does.
pub async fn fill_cart(user: &mut GooseUser) -> TransactionResult {
    let lines: usize = rand::rng().random_range(1..=3);
    for _ in 0..lines {
        let product = random_product();
        let quantity = rand::rng().random_range(1..=2);
        woocommerce::add_to_cart(user, product, quantity).await?;
        let mut basket = basket(user);
        basket.lines += 1;
        user.set_session_data(basket);
    }
    Ok(())
}

/// A returning customer's last name and email address as its account has them
/// on file (`setup.sh` gives `customerN` "Casey Customer N" and
/// `customerN@example.com`, at the address `Checkout` defaults to), so the block
/// checkout shows them filled in and the order sends no changes.
pub fn on_file(user: &GooseUser) -> (String, String) {
    let (username, _) = customer(user);
    let number = username.trim_start_matches("customer");
    (
        format!("Customer {}", number).trim_end().to_string(),
        format!("{}@example.com", username),
    )
}

/// Check out this user's cart with Cash on delivery, as a shopper does: go to
/// checkout, then place the order. A cart that is empty is filled first.
pub async fn check_out(user: &mut GooseUser, returning: bool) -> TransactionResult {
    if basket(user).lines == 0 {
        fill_cart(user).await?;
    }
    let page = woocommerce::checkout_page(user, &shop()).await?;
    let (last_name, email) = on_file(user);
    let mut checkout = Checkout::builder();
    if returning {
        checkout = checkout
            .billing("first_name", "Casey")
            .billing("last_name", &last_name)
            .billing("email", &email);
    }
    woocommerce::place_order(user, &shop(), &page, &checkout.build()).await?;
    user.set_session_data(Basket::default());
    Ok(())
}
