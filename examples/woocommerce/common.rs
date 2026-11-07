use goose::prelude::*;
use goose_eggs::woocommerce::{self, Checkout, Product, Shop};
use goose_eggs::wordpress::{self, SiteContent};
use rand::prelude::{IndexedRandom, IteratorRandom};
use rand::Rng;
use std::env;
use std::sync::OnceLock;

/// What the shop sells and how its categories are reached, found once before
/// the load test starts and read by every user.
pub struct Catalog {
    pub products: Vec<Product>,
    pub content: SiteContent,
}

/// The catalog, or why it could not be found. Set once, by `discover_catalog`
/// before any user starts, and only read after: no user ever waits on it.
static CATALOG: OnceLock<Result<Catalog, String>> = OnceLock::new();

/// The shop, on WooCommerce's default paths.
pub fn shop() -> Shop<'static> {
    Shop::default()
}

/// Find the shop's products from the cards on its shop pages, and its category
/// archives by browsing it from the home page, as a visitor would.
///
/// Registered with `GooseAttack::test_start`, so Goose runs it once, on a user
/// of its own, before any user starts: its requests are in neither the metrics
/// nor the request log, and every user starts with the catalog in hand. Goose
/// ignores what a `test_start` transaction returns, so a failure is kept with
/// the catalog, logged here, and reported by every user's `catalog_ready`.
pub async fn discover_catalog(user: &mut GooseUser) -> TransactionResult {
    let found = find_catalog(user)
        .await
        .map_err(|error| format!("discovering the catalog failed: {}", describe(&error)));
    if let Err(error) = &found {
        log::error!("{}", error);
    }
    // A Controller that starts the load test again runs this again: the first
    // catalog is kept.
    let _ = CATALOG.set(found);
    // Goose ignores this result; returned for a Goose that reads it. The
    // killswitch waits for the users: each reports the failure first.
    match CATALOG.get() {
        Some(Err(error)) => Err(Box::new(TransactionError::Custom(error.clone()))),
        _ => Ok(()),
    }
}

/// What went wrong, naming the failed request and its error.
fn describe(error: &TransactionError) -> String {
    match error {
        TransactionError::RequestFailed { raw_request } => format!(
            "{} {}: {}",
            raw_request.raw.method, raw_request.name, raw_request.error
        ),
        error => error.to_string(),
    }
}

async fn find_catalog(user: &mut GooseUser) -> Result<Catalog, Box<TransactionError>> {
    let products = woocommerce::discover_products(user, &shop(), 500).await?;
    let discover = wordpress::Discover::builder().max_per_kind(50).build();
    let content = wordpress::discover(user, &discover).await?;
    Ok(Catalog { products, content })
}

/// Each user's first transaction: fails, and stops the load test, when the
/// catalog could not be found, so the run reports why at once instead of
/// running without one.
pub async fn catalog_ready(_user: &mut GooseUser) -> TransactionResult {
    catalog().map(|_| ())
}

/// The catalog `discover_catalog` found. Without one, stops the load test and
/// fails with the reason: every transaction starts here, so none runs without it.
pub fn catalog() -> Result<&'static Catalog, Box<TransactionError>> {
    let error = match CATALOG.get() {
        Some(Ok(catalog)) => return Ok(catalog),
        Some(Err(error)) => error.as_str(),
        None => "no catalog: discover_catalog runs as the load test's test_start",
    };
    goose::trigger_killswitch(error);
    Err(Box::new(TransactionError::Custom(error.to_string())))
}

/// A random product that can go in the cart.
pub fn random_product() -> Result<&'static Product, Box<TransactionError>> {
    woocommerce::random_product(&catalog()?.products).ok_or_else(|| {
        Box::new(TransactionError::Custom(
            "the shop sells no simple product to add to the cart".to_string(),
        ))
    })
}

/// A word from a random product's name, to search for.
pub fn random_search_word() -> Result<String, Box<TransactionError>> {
    let product = catalog()?
        .products
        .choose(&mut rand::rng())
        .ok_or_else(|| {
            Box::new(TransactionError::Custom(
                "the shop has no products".to_string(),
            ))
        })?;
    Ok(product
        .name
        .split_whitespace()
        .choose(&mut rand::rng())
        .unwrap_or("shirt")
        .to_lowercase())
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
        let product = random_product()?;
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
    catalog()?;
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
