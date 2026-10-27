//! Functionality that's specific to WooCommerce.
//!
//! The helpers here drive any WooCommerce shop: browsing its shop, products and
//! search, the Store API cart and checkout that WooCommerce's block cart and
//! checkout use, the classic shortcode cart and checkout, and My Account. They
//! build on [`crate::wordpress`]. None of them assume a product, a theme or a
//! page path beyond WooCommerce's defaults, which [`ShopBuilder`] changes.

use crate::wordpress::{
    fail, form_controls, get_form, get_form_by_class, get_named, header_value, json_response,
    load_page, plain_text, query_encode, read_response, request_ok, send_json, site_path,
};
use goose::metrics::GooseRequestMetric;
use goose::prelude::*;
use rand::seq::IndexedRandom;
use regex::Regex;

/// Where a shop's pages and Store API are. For complete documentation, refer to
/// [`ShopBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::Shop;
///
/// let shop: Shop = Shop::builder().my_account("account/").build();
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::my_account_site("account/", "customer", "secret");
/// # support::my_account_log_in("Shop", &server, &shop, "customer", "secret");
/// ```
#[derive(Clone, Debug)]
pub struct Shop<'a> {
    shop: &'a str,
    cart: &'a str,
    checkout: &'a str,
    my_account: &'a str,
    orders: &'a str,
    store_api: &'a str,
}
impl<'a> Shop<'a> {
    /// Convenience function to bring [`ShopBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().shop("store/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::shop_site("store/");
    /// # support::shop_page("Shop::builder", &server, &_shop);
    /// ```
    pub fn builder() -> ShopBuilder<'a> {
        ShopBuilder::new()
    }
}
impl Default for Shop<'_> {
    /// A shop on WooCommerce's default paths.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::default();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::shop_site("shop/");
    /// # support::shop_page("Shop::default", &server, &_shop);
    /// ```
    fn default() -> Self {
        ShopBuilder::new().build()
    }
}

/// Used to build a [`Shop`], which every function in this module takes.
///
/// The defaults are the pages WooCommerce's setup creates. A shop on the classic
/// shortcode cart and checkout often has them elsewhere.
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::Shop;
///
/// // A shop with its classic cart and checkout on their own pages.
/// let _shop = Shop::builder()
///     .cart("classic-cart/")
///     .checkout("classic-checkout/")
///     .build();
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::classic_cart_site("classic-cart/");
/// # support::classic_update_cart("ShopBuilder", &server, &_shop);
/// ```
pub struct ShopBuilder<'a> {
    shop: &'a str,
    cart: &'a str,
    checkout: &'a str,
    my_account: &'a str,
    orders: &'a str,
    store_api: &'a str,
}
impl<'a> ShopBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            shop: "shop/",
            cart: "cart/",
            checkout: "checkout/",
            my_account: "my-account/",
            orders: "my-account/orders/",
            store_api: "wp-json/wc/store/v1/",
        }
    }

    /// The shop page, the product archive.
    ///
    /// Defaults to `shop/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().shop("products/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::shop_site("products/");
    /// # support::shop_page("ShopBuilder::shop", &server, &_shop);
    /// ```
    pub fn shop(mut self, shop: &'a str) -> Self {
        self.shop = shop;
        self
    }

    /// The cart page, used by the classic flows.
    ///
    /// Defaults to `cart/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().cart("basket/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::classic_cart_site("basket/");
    /// # support::classic_update_cart("ShopBuilder::cart", &server, &_shop);
    /// ```
    pub fn cart(mut self, cart: &'a str) -> Self {
        self.cart = cart;
        self
    }

    /// The checkout page, used by [`classic_checkout`].
    ///
    /// Defaults to `checkout/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().checkout("pay/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::classic_checkout_site("pay/", "cod");
    /// # support::classic_checkout("ShopBuilder::checkout", &server, &_shop);
    /// ```
    pub fn checkout(mut self, checkout: &'a str) -> Self {
        self.checkout = checkout;
        self
    }

    /// The My Account page, with its log in form.
    ///
    /// Defaults to `my-account/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().my_account("account/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::my_account_site("account/", "customer", "password");
    /// # support::my_account_log_in("ShopBuilder::my_account", &server, &_shop, "customer", "password");
    /// ```
    pub fn my_account(mut self, my_account: &'a str) -> Self {
        self.my_account = my_account;
        self
    }

    /// My Account's orders list.
    ///
    /// Defaults to `my-account/orders/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().orders("account/my-orders/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::orders_site("account/my-orders/");
    /// # support::my_account_orders("ShopBuilder::orders", &server, &_shop);
    /// ```
    pub fn orders(mut self, orders: &'a str) -> Self {
        self.orders = orders;
        self
    }

    /// The Store API root.
    ///
    /// Defaults to `wp-json/wc/store/v1/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().store_api("api/wc/store/v1/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_api_site("api/wc/store/v1/");
    /// # support::discover_products("ShopBuilder::store_api", &server, &_shop);
    /// ```
    pub fn store_api(mut self, store_api: &'a str) -> Self {
        self.store_api = store_api;
        self
    }

    /// Build the [`Shop`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::shop_site("shop/");
    /// # support::shop_page("ShopBuilder::build", &server, &_shop);
    /// ```
    pub fn build(self) -> Shop<'a> {
        let Self {
            shop,
            cart,
            checkout,
            my_account,
            orders,
            store_api,
        } = self;
        Shop {
            shop,
            cart,
            checkout,
            my_account,
            orders,
            store_api,
        }
    }
}

/// Load the shop page.
///
/// Checks the page is the product archive (WordPress's `post-type-archive-product`
/// body class) unless `validate` is given, and loads its static assets. Returns
/// the page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{shop_page, Shop};
///
/// transaction!(browse);
///
/// async fn browse(user: &mut GooseUser) -> TransactionResult {
///     let _html = shop_page(user, &Shop::default(), None).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::shop_site("shop/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(browse(&mut user));
/// # assert!(result.is_ok(), "doc example shop_page: {}", support::failure(&result));
/// # let result = support::block_on(async { shop_page(&mut user, &Shop::builder().shop("missing/").build(), None).await });
/// # assert!(result.is_err(), "doc example shop_page: a missing shop page passed");
/// ```
pub async fn shop_page(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    validate: Option<&crate::Validate<'_>>,
) -> Result<String, Box<TransactionError>> {
    let default_validation = crate::Validate::builder()
        .text("post-type-archive-product")
        .build();
    let goose = user.get(shop.shop).await?;
    load_page(user, goose, Some(validate.unwrap_or(&default_validation))).await
}

/// Load a product's page, by its path (a [`Product::url`]).
///
/// Checks the page is a single product (WordPress's `single-product` body
/// class) unless `validate` is given, and loads its static assets. The request
/// is named `product page`, so every product shares one line in the metrics.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::product_page;
///
/// transaction!(view);
///
/// async fn view(user: &mut GooseUser) -> TransactionResult {
///     let _html = product_page(user, "/product/beanie/", None).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::product_site("/product/beanie/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(view(&mut user));
/// # assert!(result.is_ok(), "doc example product_page: {}", support::failure(&result));
/// # let result = support::block_on(async { product_page(&mut user, "/shop/", None).await });
/// # assert!(result.is_err(), "doc example product_page: the shop page passed as a product");
/// ```
pub async fn product_page(
    user: &mut GooseUser,
    path: &str,
    validate: Option<&crate::Validate<'_>>,
) -> Result<String, Box<TransactionError>> {
    let default_validation = crate::Validate::builder().text("single-product").build();
    let goose = get_named(user, path, "product page").await?;
    load_page(user, goose, Some(validate.unwrap_or(&default_validation))).await
}

/// Search the shop's products, as WooCommerce's product search does:
/// `?s=<keys>&post_type=product` on the home page.
///
/// The request is named `product search`, whatever the keys. Returns the results.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::product_search;
///
/// transaction!(find);
///
/// async fn find(user: &mut GooseUser) -> TransactionResult {
///     let _results = product_search(user, "hoodie", None).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::product_search_site("hoodie");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(find(&mut user));
/// # assert!(result.is_ok(), "doc example product_search: {}", support::failure(&result));
/// # assert_eq!(support::searches(&server), 1, "doc example product_search: searches");
/// ```
pub async fn product_search(
    user: &mut GooseUser,
    keys: &str,
    validate: Option<&crate::Validate<'_>>,
) -> Result<String, Box<TransactionError>> {
    let path = format!("?s={}&post_type=product", query_encode(keys));
    let goose = get_named(user, &path, "product search").await?;
    load_page(user, goose, validate).await
}

/// A product, as the Store API lists it.
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::Product;
///
/// let product = Product {
///     id: 31,
///     name: "Beanie with Logo".to_string(),
///     url: "/product/beanie-with-logo/".to_string(),
///     kind: "simple".to_string(),
///     purchasable: true,
///     in_stock: true,
///     has_options: false,
/// };
/// assert!(product.can_add_to_cart());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Product {
    /// The product ID, as the cart takes it.
    pub id: u64,
    /// The product name.
    pub name: String,
    /// The product page, as a path on the shop.
    pub url: String,
    /// The product type: `simple`, `variable`, `grouped`, `external`, or a plugin's.
    pub kind: String,
    /// Whether the product can be bought at all.
    pub purchasable: bool,
    /// Whether the product is in stock.
    pub in_stock: bool,
    /// Whether adding the product needs a choice first, such as a variation.
    pub has_options: bool,
}
impl Product {
    /// Whether the product can go in the cart by its ID alone: purchasable, in
    /// stock, and without options to choose.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Product;
    ///
    /// let mut product = Product {
    ///     id: 11,
    ///     name: "Hoodie".to_string(),
    ///     url: "/product/hoodie/".to_string(),
    ///     kind: "variable".to_string(),
    ///     purchasable: true,
    ///     in_stock: true,
    ///     has_options: true,
    /// };
    /// // A variable product needs a variation chosen first.
    /// assert!(!product.can_add_to_cart());
    /// product.has_options = false;
    /// product.in_stock = false;
    /// assert!(!product.can_add_to_cart());
    /// ```
    pub fn can_add_to_cart(&self) -> bool {
        self.purchasable && self.in_stock && !self.has_options
    }
}

/// Find a shop's products through the Store API, up to `max`, following
/// `X-WP-TotalPages` 100 at a time.
///
/// Discovery makes several requests, so run it once per load test and share the
/// result, as with [`crate::wordpress::discover`].
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{discover_products, random_product, Shop};
///
/// transaction!(find_products).set_on_start();
///
/// async fn find_products(user: &mut GooseUser) -> TransactionResult {
///     let products = discover_products(user, &Shop::default(), 500).await?;
///     let product = random_product(&products).expect("a product to buy");
///     assert!(product.can_add_to_cart());
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::store_api_site("wp-json/wc/store/v1/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(find_products(&mut user));
/// # assert!(result.is_ok(), "doc example discover_products: {}", support::failure(&result));
/// ```
pub async fn discover_products(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    max: usize,
) -> Result<Vec<Product>, Box<TransactionError>> {
    let name = format!("{}products", shop.store_api);
    let mut products = Vec::new();
    // One page size throughout, so the pages line up.
    let per_page = max.min(100);
    let mut page = 1;
    while products.len() < max {
        let path = format!(
            "{}products?per_page={}&page={}",
            shop.store_api, per_page, page
        );
        let goose = get_named(user, &path, &name).await?;
        let (headers, json) = json_response(user, goose).await?;
        let listed = json.as_array().cloned().unwrap_or_default();
        for item in &listed {
            if let Some(product) = parse_product(item) {
                products.push(product);
            }
        }
        let pages = header_value(&headers, "X-WP-TotalPages")
            .and_then(|p| p.parse::<usize>().ok())
            .unwrap_or(1);
        if listed.is_empty() || page >= pages {
            break;
        }
        page += 1;
    }
    products.truncate(max);
    Ok(products)
}

// A product from a Store API product listing.
fn parse_product(item: &serde_json::Value) -> Option<Product> {
    let text = |key: &str| item.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let flag = |key: &str| item.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    Some(Product {
        id: item.get("id")?.as_u64()?,
        name: text("name").unwrap_or_default(),
        url: site_path(&text("permalink")?),
        kind: text("type").unwrap_or_default(),
        purchasable: flag("is_purchasable"),
        in_stock: flag("is_in_stock"),
        has_options: flag("has_options"),
    })
}

/// A product that [`Product::can_add_to_cart`], picked at random; `None` when
/// there is none.
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::{random_product, Product};
///
/// let product = |id: u64, has_options: bool| Product {
///     id,
///     name: format!("Product {}", id),
///     url: format!("/product/{}/", id),
///     kind: "simple".to_string(),
///     purchasable: true,
///     in_stock: true,
///     has_options,
/// };
/// let products = [product(10, true), product(31, false)];
/// assert_eq!(random_product(&products).map(|p| p.id), Some(31));
/// assert_eq!(random_product(&products[..1]), None);
/// ```
pub fn random_product(products: &[Product]) -> Option<&Product> {
    let buyable: Vec<&Product> = products.iter().filter(|p| p.can_add_to_cart()).collect();
    buyable.choose(&mut rand::rng()).copied()
}

/// A line in a Store API cart.
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::{CartItem, Shop, StoreApiCart};
///
/// let mut cart = StoreApiCart::new(&Shop::default());
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::store_cart_site();
/// # support::cart_add("CartItem", &server, &mut cart);
/// // After adding two of product 31:
/// let item: &CartItem = &cart.items()[0];
/// assert_eq!(item.key, "c16a5320fa475530d9583c34fd356ef5");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartItem {
    /// The cart item key, which `update_item` and `remove_item` take.
    pub key: String,
    /// The product ID.
    pub id: u64,
    /// How many are in the cart.
    pub quantity: u64,
}

/// A user's cart through the Store API, the API WooCommerce's block cart and
/// checkout use.
///
/// Store API writes must send back the `Nonce` header the API last answered
/// with, along with the session cookie; the cart keeps it, refreshes it from every
/// response, and loads the cart first when it has none. It also keeps the
/// `Cart-Token`, which a client without cookies sends instead; WooCommerce does
/// not check the nonce when a cart token is sent, so the cart only sends it when
/// asked to ([`StoreApiCart::use_cart_token`]).
///
/// Keep one cart per Goose user, in the user's session data. A cart and its user
/// cannot be borrowed at once, so take a copy out and put it back:
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{Shop, StoreApiCart};
///
/// transaction!(add_to_cart);
///
/// async fn add_to_cart(user: &mut GooseUser) -> TransactionResult {
///     let mut cart = user
///         .get_session_data::<StoreApiCart>()
///         .cloned()
///         .unwrap_or_else(|| StoreApiCart::new(&Shop::default()));
///     cart.add_item(user, 31, 2).await?;
///     user.set_session_data(cart);
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::store_cart_site();
/// # let mut user = support::user(&server);
/// # let result = support::block_on(add_to_cart(&mut user));
/// # assert!(result.is_ok(), "doc example StoreApiCart: {}", support::failure(&result));
/// # assert_eq!(user.get_session_data::<StoreApiCart>().map(|c| c.items().len()), Some(1), "doc example StoreApiCart: items");
/// ```
#[derive(Clone, Debug)]
pub struct StoreApiCart {
    store_api: String,
    nonce: Option<String>,
    cart_token: Option<String>,
    use_cart_token: bool,
    items: Vec<CartItem>,
}
impl StoreApiCart {
    /// An empty cart on `shop`'s Store API, with no nonce yet.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// let cart = StoreApiCart::new(&Shop::default());
    /// assert_eq!(cart.nonce(), None);
    /// assert!(cart.items().is_empty());
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut cart = cart;
    /// # support::cart_load("StoreApiCart::new", &server, &mut cart);
    /// ```
    pub fn new(shop: &Shop<'_>) -> StoreApiCart {
        StoreApiCart {
            store_api: shop.store_api.to_string(),
            nonce: None,
            cart_token: None,
            use_cart_token: false,
            items: Vec::new(),
        }
    }

    /// Send the `Cart-Token` instead of relying on the session cookie, as a
    /// headless client does. WooCommerce then skips the nonce check.
    ///
    /// Defaults to `false`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// let _cart = StoreApiCart::new(&Shop::default()).use_cart_token(true);
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_token_site();
    /// # let mut cart = _cart;
    /// # support::cart_add("StoreApiCart::use_cart_token", &server, &mut cart);
    /// ```
    pub fn use_cart_token(mut self, use_cart_token: bool) -> StoreApiCart {
        self.use_cart_token = use_cart_token;
        self
    }

    /// The nonce the Store API last answered with.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// let mut cart = StoreApiCart::new(&Shop::default());
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # support::cart_load("StoreApiCart::nonce", &server, &mut cart);
    /// // After the cart is loaded, it holds the Store API's nonce.
    /// assert_eq!(cart.nonce(), Some(support::nonce().as_str()));
    /// ```
    pub fn nonce(&self) -> Option<&str> {
        self.nonce.as_deref()
    }

    /// The cart token the Store API last answered with.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// let mut cart = StoreApiCart::new(&Shop::default());
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # support::cart_load("StoreApiCart::cart_token", &server, &mut cart);
    /// assert!(cart.cart_token().is_some_and(|token| token.starts_with("eyJ")));
    /// ```
    pub fn cart_token(&self) -> Option<&str> {
        self.cart_token.as_deref()
    }

    /// The cart's lines, as the Store API last returned them.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// let mut cart = StoreApiCart::new(&Shop::default());
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # support::cart_add("StoreApiCart::items", &server, &mut cart);
    /// // After adding two of product 31:
    /// assert_eq!(cart.items()[0].id, 31);
    /// assert_eq!(cart.items()[0].quantity, 2);
    /// ```
    pub fn items(&self) -> &[CartItem] {
        &self.items
    }

    // Send one Store API request, keeping the nonce, cart token and items it returns.
    async fn send(
        &mut self,
        user: &mut GooseUser,
        method: GooseMethod,
        route: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(GooseRequestMetric, serde_json::Value), Box<TransactionError>> {
        // Writes need a nonce; loading the cart gets one.
        if body.is_some() && self.nonce.is_none() {
            Box::pin(self.load(user)).await?;
        }
        let path = format!("{}{}", self.store_api, route);
        let mut headers = Vec::new();
        if let Some(nonce) = self.nonce.as_deref() {
            headers.push(("Nonce", nonce));
        }
        if self.use_cart_token {
            if let Some(token) = self.cart_token.as_deref() {
                headers.push(("Cart-Token", token));
            }
        }
        let goose = send_json(user, method, &path, &path, &headers, body).await?;
        let request = goose.request.clone();
        let (headers, json) = json_response(user, goose).await?;
        if let Some(nonce) = header_value(&headers, "Nonce") {
            self.nonce = Some(nonce);
        }
        if let Some(token) = header_value(&headers, "Cart-Token") {
            self.cart_token = Some(token);
        }
        if let Some(items) = json.get("items").and_then(|i| i.as_array()) {
            self.items = items
                .iter()
                .filter_map(|item| {
                    Some(CartItem {
                        key: item.get("key")?.as_str()?.to_string(),
                        id: item.get("id")?.as_u64()?,
                        quantity: item.get("quantity")?.as_u64()?,
                    })
                })
                .collect();
        }
        Ok((request, json))
    }

    /// Load the cart, `GET cart`, which also gets the nonce writes need.
    ///
    /// Load it again after logging in: the nonce belongs to the user it was
    /// given to. Returns the cart, as JSON.
    ///
    /// # Example
    /// ```rust
    /// use goose::prelude::*;
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// transaction!(view_cart);
    ///
    /// async fn view_cart(user: &mut GooseUser) -> TransactionResult {
    ///     let mut cart = StoreApiCart::new(&Shop::default());
    ///     let json = cart.load(user).await?;
    ///     assert!(json["items"].is_array());
    ///     user.set_session_data(cart);
    ///     Ok(())
    /// }
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut user = support::user(&server);
    /// # let result = support::block_on(view_cart(&mut user));
    /// # assert!(result.is_ok(), "doc example load: {}", support::failure(&result));
    /// # assert_eq!(user.get_session_data::<StoreApiCart>().and_then(|c| c.nonce().map(str::to_string)).as_deref(), Some(support::nonce().as_str()), "doc example load: nonce");
    /// ```
    pub async fn load(
        &mut self,
        user: &mut GooseUser,
    ) -> Result<serde_json::Value, Box<TransactionError>> {
        self.send(user, GooseMethod::Get, "cart", None)
            .await
            .map(|(_, json)| json)
    }

    /// Add `quantity` of product `id` to the cart, `POST cart/add-item`.
    ///
    /// Fails when WooCommerce refuses, for example a 400 for a product that does
    /// not exist (`woocommerce_rest_cart_invalid_product`), is out of stock
    /// (`woocommerce_rest_product_out_of_stock`), or needs a variation chosen.
    /// Returns the cart.
    ///
    /// # Example
    /// ```rust
    /// use goose::prelude::*;
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// transaction!(buy_two);
    ///
    /// async fn buy_two(user: &mut GooseUser) -> TransactionResult {
    ///     let mut cart = StoreApiCart::new(&Shop::default());
    ///     cart.add_item(user, 31, 2).await?;
    ///     user.set_session_data(cart);
    ///     Ok(())
    /// }
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut user = support::user(&server);
    /// # let result = support::block_on(buy_two(&mut user));
    /// # assert!(result.is_ok(), "doc example add_item: {}", support::failure(&result));
    /// # assert_eq!(support::adds(&server), 1, "doc example add_item: items added with the nonce");
    /// ```
    pub async fn add_item(
        &mut self,
        user: &mut GooseUser,
        id: u64,
        quantity: u64,
    ) -> Result<serde_json::Value, Box<TransactionError>> {
        let body = serde_json::json!({ "id": id, "quantity": quantity });
        self.send(user, GooseMethod::Post, "cart/add-item", Some(&body))
            .await
            .map(|(_, json)| json)
    }

    /// Change a cart line's quantity, `POST cart/update-item`, by its
    /// [`CartItem::key`]. Returns the cart.
    ///
    /// # Example
    /// ```rust
    /// use goose::prelude::*;
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// transaction!(buy_more);
    ///
    /// async fn buy_more(user: &mut GooseUser) -> TransactionResult {
    ///     let mut cart = StoreApiCart::new(&Shop::default());
    ///     cart.add_item(user, 31, 2).await?;
    ///     let key = cart.items()[0].key.clone();
    ///     cart.update_item(user, &key, 3).await?;
    ///     assert_eq!(cart.items()[0].quantity, 3);
    ///     Ok(())
    /// }
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut user = support::user(&server);
    /// # let result = support::block_on(buy_more(&mut user));
    /// # assert!(result.is_ok(), "doc example update_item: {}", support::failure(&result));
    /// ```
    pub async fn update_item(
        &mut self,
        user: &mut GooseUser,
        key: &str,
        quantity: u64,
    ) -> Result<serde_json::Value, Box<TransactionError>> {
        let body = serde_json::json!({ "key": key, "quantity": quantity });
        self.send(user, GooseMethod::Post, "cart/update-item", Some(&body))
            .await
            .map(|(_, json)| json)
    }

    /// Remove a cart line, `POST cart/remove-item`, by its [`CartItem::key`].
    ///
    /// Fails with a 409 (`woocommerce_rest_cart_invalid_key`) when the line is
    /// no longer in the cart. Returns the cart.
    ///
    /// # Example
    /// ```rust
    /// use goose::prelude::*;
    /// use goose_eggs::woocommerce::{Shop, StoreApiCart};
    ///
    /// transaction!(change_mind);
    ///
    /// async fn change_mind(user: &mut GooseUser) -> TransactionResult {
    ///     let mut cart = StoreApiCart::new(&Shop::default());
    ///     cart.add_item(user, 31, 2).await?;
    ///     let key = cart.items()[0].key.clone();
    ///     cart.remove_item(user, &key).await?;
    ///     assert!(cart.items().is_empty());
    ///     Ok(())
    /// }
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut user = support::user(&server);
    /// # let result = support::block_on(change_mind(&mut user));
    /// # assert!(result.is_ok(), "doc example remove_item: {}", support::failure(&result));
    /// ```
    pub async fn remove_item(
        &mut self,
        user: &mut GooseUser,
        key: &str,
    ) -> Result<serde_json::Value, Box<TransactionError>> {
        let body = serde_json::json!({ "key": key });
        self.send(user, GooseMethod::Post, "cart/remove-item", Some(&body))
            .await
            .map(|(_, json)| json)
    }

    /// Check out the cart, `POST checkout`, with the billing address (also used
    /// for shipping) and payment method from `checkout`.
    ///
    /// Fails when WooCommerce refuses the checkout (a 400, for example
    /// `rest_invalid_param` for a bad address), or answers without an order or
    /// with a payment that did not succeed. Returns the order, as JSON.
    ///
    /// # Example
    /// ```rust
    /// use goose::prelude::*;
    /// use goose_eggs::woocommerce::{Checkout, Shop, StoreApiCart};
    ///
    /// transaction!(buy);
    ///
    /// async fn buy(user: &mut GooseUser) -> TransactionResult {
    ///     let mut cart = StoreApiCart::new(&Shop::default());
    ///     cart.add_item(user, 31, 1).await?;
    ///     let order = cart.checkout(user, &Checkout::builder().build()).await?;
    ///     assert!(order["order_id"].as_u64().is_some());
    ///     Ok(())
    /// }
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::store_cart_site();
    /// # let mut user = support::user(&server);
    /// # let result = support::block_on(buy(&mut user));
    /// # assert!(result.is_ok(), "doc example checkout: {}", support::failure(&result));
    /// # assert_eq!(support::checkouts(&server), 1, "doc example checkout: orders placed");
    /// ```
    pub async fn checkout(
        &mut self,
        user: &mut GooseUser,
        checkout: &Checkout<'_>,
    ) -> Result<serde_json::Value, Box<TransactionError>> {
        let address = serde_json::Value::Object(
            checkout
                .billing
                .iter()
                .map(|(k, v)| (k.to_string(), serde_json::Value::from(*v)))
                .collect(),
        );
        let mut shipping = address.clone();
        if let Some(shipping) = shipping.as_object_mut() {
            shipping.remove("email");
        }
        let body = serde_json::json!({
            "billing_address": address,
            "shipping_address": shipping,
            "payment_method": checkout.payment_method,
        });
        let (mut request, order) = self
            .send(user, GooseMethod::Post, "checkout", Some(&body))
            .await?;
        let status = order["payment_result"]["payment_status"]
            .as_str()
            .unwrap_or("none");
        if order["order_id"].as_u64().is_none() || !matches!(status, "success" | "pending") {
            return fail(
                user,
                &format!(
                    "{}checkout: order not placed: payment {}",
                    self.store_api, status
                ),
                &mut request,
                None,
                Some(&order.to_string()),
            );
        }
        self.items.clear();
        Ok(order)
    }
}

/// The details a checkout sends. For complete documentation, refer to
/// [`CheckoutBuilder`].
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::Checkout;
///
/// let checkout = Checkout::builder().build();
/// assert_eq!(checkout.payment_method(), "cod");
/// ```
#[derive(Clone, Debug)]
pub struct Checkout<'a> {
    billing: Vec<(&'a str, &'a str)>,
    payment_method: &'a str,
}
impl<'a> Checkout<'a> {
    /// Convenience function to bring [`CheckoutBuilder`] into scope.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().billing("email", "ada@example.com").build();
    /// assert_eq!(checkout.billing("email"), Some("ada@example.com"));
    /// // The rest of the address is the builder's default.
    /// assert_eq!(checkout.billing("postcode"), Some("94105"));
    /// ```
    pub fn builder() -> CheckoutBuilder<'a> {
        CheckoutBuilder::new()
    }

    /// A billing field, by its Store API name (`first_name`, `email`, ...).
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().build();
    /// assert_eq!(checkout.billing("country"), Some("US"));
    /// assert_eq!(checkout.billing("fax"), None);
    /// ```
    pub fn billing(&self, field: &str) -> Option<&'a str> {
        self.billing
            .iter()
            .find(|(name, _)| *name == field)
            .map(|(_, value)| *value)
    }

    /// The payment method's gateway ID.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().payment_method("cheque").build();
    /// assert_eq!(checkout.payment_method(), "cheque");
    /// ```
    pub fn payment_method(&self) -> &'a str {
        self.payment_method
    }
}

/// Used to build a [`Checkout`], which [`StoreApiCart::checkout`] and
/// [`classic_checkout`] take.
///
/// The default is a complete US address and Cash on delivery (`cod`), which
/// needs no outside service; the gateway must be enabled in the shop.
///
/// # Example
/// ```rust
/// use goose_eggs::woocommerce::Checkout;
///
/// let checkout = Checkout::builder()
///     .billing("first_name", "Ada")
///     .billing("email", "ada@example.com")
///     .payment_method("cod")
///     .build();
/// assert_eq!(checkout.billing("first_name"), Some("Ada"));
/// assert_eq!(checkout.billing("city"), Some("San Francisco"));
/// ```
pub struct CheckoutBuilder<'a> {
    billing: Vec<(&'a str, &'a str)>,
    payment_method: &'a str,
}
impl<'a> CheckoutBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            billing: vec![
                ("first_name", "Goose"),
                ("last_name", "Shopper"),
                ("address_1", "1 Main Street"),
                ("city", "San Francisco"),
                ("state", "CA"),
                ("postcode", "94105"),
                ("country", "US"),
                ("email", "goose@example.com"),
                ("phone", "5555550100"),
            ],
            payment_method: "cod",
        }
    }

    /// Set a billing field, by its Store API name: `first_name`, `last_name`,
    /// `company`, `address_1`, `address_2`, `city`, `state`, `postcode`,
    /// `country`, `email` or `phone`. The classic checkout sends it as
    /// `billing_<name>`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().billing("city", "Austin").billing("state", "TX").build();
    /// assert_eq!(checkout.billing("city"), Some("Austin"));
    /// assert_eq!(checkout.billing("state"), Some("TX"));
    /// ```
    pub fn billing(mut self, field: &'a str, value: &'a str) -> Self {
        match self.billing.iter_mut().find(|(name, _)| *name == field) {
            Some(entry) => entry.1 = value,
            None => self.billing.push((field, value)),
        }
        self
    }

    /// The payment method's gateway ID.
    ///
    /// Defaults to `cod`, Cash on delivery.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().payment_method("bacs").build();
    /// assert_eq!(checkout.payment_method(), "bacs");
    /// ```
    pub fn payment_method(mut self, payment_method: &'a str) -> Self {
        self.payment_method = payment_method;
        self
    }

    /// Build the [`Checkout`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Checkout;
    ///
    /// let checkout = Checkout::builder().build();
    /// assert_eq!(checkout.billing("email"), Some("goose@example.com"));
    /// ```
    pub fn build(self) -> Checkout<'a> {
        let Self {
            billing,
            payment_method,
        } = self;
        Checkout {
            billing,
            payment_method,
        }
    }
}

// The text of the first error notice on a page, classic (`woocommerce-error`)
// or block (`is-error`) themed.
fn error_notice(html: &str) -> Option<String> {
    let re = Regex::new(
        r#"(?s)class="[^"]*(?:woocommerce-error|is-error)[^"]*"[^>]*>(.*?)</(?:ul|div)>\s*(?:</div>)?"#,
    )
    .unwrap();
    re.captures(html).map(|c| plain_text(&c[1]))
}

// Whether a page shows a success notice, classic or block themed.
fn has_success_notice(html: &str) -> bool {
    Regex::new(r#"class="[^"]*(?:woocommerce-message|is-success)[^"]*""#)
        .unwrap()
        .is_match(html)
}

// Load a classic page and fail unless it shows a success notice.
async fn expect_success_notice(
    user: &mut GooseUser,
    goose: goose::goose::GooseResponse,
    what: &str,
) -> Result<String, Box<TransactionError>> {
    let mut request = goose.request.clone();
    let html = load_page(user, goose, None).await?;
    if has_success_notice(&html) && error_notice(&html).is_none() {
        return Ok(html);
    }
    let reason = error_notice(&html).unwrap_or_else(|| "no success notice".to_string());
    fail(
        user,
        &format!("{}: {}: {}", request.raw.url, what, reason),
        &mut request,
        None,
        Some(&html),
    )
}

/// Add a product to the cart the classic way, `?add-to-cart=<id>&quantity=<n>`
/// on the cart page, as a shop's add to cart links do.
///
/// Fails unless WooCommerce shows its "added to your cart" success notice; a
/// product that does not exist adds nothing and shows no notice. The request is
/// named `classic add to cart`. Returns the cart page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{classic_add_to_cart, Shop};
///
/// transaction!(add);
///
/// async fn add(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::builder().cart("classic-cart/").build();
///     let _cart = classic_add_to_cart(user, &shop, 31, 2).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::classic_add_site("classic-cart/", 31, 2);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(add(&mut user));
/// # assert!(result.is_ok(), "doc example classic_add_to_cart: {}", support::failure(&result));
/// ```
pub async fn classic_add_to_cart(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    id: u64,
    quantity: u64,
) -> Result<String, Box<TransactionError>> {
    let path = format!("{}?add-to-cart={}&quantity={}", shop.cart, id, quantity);
    let goose = get_named(user, &path, "classic add to cart").await?;
    expect_success_notice(user, goose, "not added to cart").await
}

/// Set every line of the classic cart to `quantity` (0 empties the cart), by
/// posting the cart page's form with its `woocommerce-cart-nonce`.
///
/// Fails when the cart page has no cart form, or WooCommerce does not confirm
/// the update (a stale nonce is ignored without a word). Returns the cart page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{classic_update_cart, Shop};
///
/// transaction!(update);
///
/// async fn update(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::builder().cart("classic-cart/").build();
///     let _cart = classic_update_cart(user, &shop, 3).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::classic_cart_site("classic-cart/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(update(&mut user));
/// # assert!(result.is_ok(), "doc example classic_update_cart: {}", support::failure(&result));
/// ```
pub async fn classic_update_cart(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    quantity: u64,
) -> Result<String, Box<TransactionError>> {
    let goose = user.get(shop.cart).await?;
    let mut request = goose.request.clone();
    let page = load_page(user, goose, None).await?;
    let form = get_form_by_class(&page, "woocommerce-cart-form");
    let quantity_field = Regex::new(r"^cart\[[^\]]+\]\[qty\]$").unwrap();
    let mut params: Vec<(String, String)> = form_controls(&form)
        .into_iter()
        .map(|(name, value)| {
            if quantity_field.is_match(&name) {
                (name, quantity.to_string())
            } else {
                (name, value)
            }
        })
        .collect();
    if !params
        .iter()
        .any(|(name, _)| name == "woocommerce-cart-nonce")
    {
        return fail(
            user,
            &format!("{}: no cart form with a nonce on page", shop.cart),
            &mut request,
            None,
            Some(&page),
        );
    }
    params.push(("update_cart".to_string(), "Update cart".to_string()));
    let goose = user.post_form(shop.cart, &params).await?;
    expect_success_notice(user, goose, "cart not updated").await
}

/// Check out the classic cart: load the checkout page, fill its form with the
/// billing details and payment method from `checkout`, keeping the form's own
/// choices (shipping method, shipping to the billing address) and its
/// `woocommerce-process-checkout-nonce`, and post it to `?wc-ajax=checkout` as
/// WooCommerce's checkout script does.
///
/// WooCommerce answers with JSON; the checkout fails on `"result":"failure"`,
/// naming its error notices, and on success the order received page it
/// redirects to is loaded and returned.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{classic_checkout, Checkout, Shop};
///
/// transaction!(buy);
///
/// async fn buy(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::builder().checkout("classic-checkout/").build();
///     let _received = classic_checkout(user, &shop, &Checkout::builder().build()).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::classic_checkout_site("classic-checkout/", "cod");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(buy(&mut user));
/// # assert!(result.is_ok(), "doc example classic_checkout: {}", support::failure(&result));
/// ```
pub async fn classic_checkout(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    checkout: &Checkout<'_>,
) -> Result<String, Box<TransactionError>> {
    let goose = user.get(shop.checkout).await?;
    let mut request = goose.request.clone();
    let page = load_page(user, goose, None).await?;
    let form = get_form(&page, "checkout");
    let mut params = form_controls(&form);
    if !params
        .iter()
        .any(|(name, _)| name == "woocommerce-process-checkout-nonce")
    {
        return fail(
            user,
            &format!("{}: no checkout form with a nonce on page", shop.checkout),
            &mut request,
            None,
            Some(&page),
        );
    }
    let mut set = |name: String, value: &str| match params.iter_mut().find(|(n, _)| *n == name) {
        Some(entry) => entry.1 = value.to_string(),
        None => params.push((name, value.to_string())),
    };
    for (field, value) in &checkout.billing {
        set(format!("billing_{}", field), value);
    }
    set("payment_method".to_string(), checkout.payment_method);

    let goose = user.post_form("?wc-ajax=checkout", &params).await?;
    let mut request = goose.request.clone();
    let (headers, result) = json_response(user, goose).await?;
    match (result["result"].as_str(), result["redirect"].as_str()) {
        (Some("success"), Some(redirect)) => {
            let goose = get_named(user, redirect, "order received").await?;
            request_ok(&goose)?;
            load_page(user, goose, None).await
        }
        _ => fail(
            user,
            &format!(
                "?wc-ajax=checkout: checkout failed: {}",
                plain_text(result["messages"].as_str().unwrap_or("no result"))
            ),
            &mut request,
            Some(&headers),
            Some(&result.to_string()),
        ),
    }
}

/// Log in through My Account's log in form, with its `woocommerce-login-nonce`.
///
/// Fails unless WooCommerce answers with the logged in My Account; a wrong
/// password shows the form again with an error notice, which the failure names.
/// Returns the My Account page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{my_account_log_in, Shop};
///
/// transaction!(log_in).set_on_start();
///
/// async fn log_in(user: &mut GooseUser) -> TransactionResult {
///     let _account = my_account_log_in(user, &Shop::default(), "customer", "secret").await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::my_account_site("my-account/", "customer", "secret");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(log_in(&mut user));
/// # assert!(result.is_ok(), "doc example my_account_log_in: {}", support::failure(&result));
/// ```
pub async fn my_account_log_in(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    username: &str,
    password: &str,
) -> Result<String, Box<TransactionError>> {
    let goose = user.get(shop.my_account).await?;
    let mut request = goose.request.clone();
    let page = load_page(user, goose, None).await?;
    let form = get_form_by_class(&page, "woocommerce-form-login");
    let mut params = form_controls(&form);
    if !params
        .iter()
        .any(|(name, _)| name == "woocommerce-login-nonce")
    {
        return fail(
            user,
            &format!("{}: no log in form with a nonce on page", shop.my_account),
            &mut request,
            None,
            Some(&page),
        );
    }
    for (name, value) in params.iter_mut() {
        match name.as_str() {
            "username" => *value = username.to_string(),
            "password" => *value = password.to_string(),
            _ => {}
        }
    }
    params.push(("login".to_string(), "Log in".to_string()));
    let goose = user.post_form(shop.my_account, &params).await?;
    request_ok(&goose)?;
    // WooCommerce redirects a good log in back to the same page, so success is
    // the logged in My Account: its navigation, without the log in form.
    let mut request = goose.request.clone();
    let (headers, html) = read_response(goose).await;
    if html.contains("woocommerce-MyAccount-navigation") && !html.contains("woocommerce-form-login")
    {
        crate::load_static_elements(user, &html).await;
        return Ok(html);
    }
    let reason = error_notice(&html).unwrap_or_else(|| "not logged in".to_string());
    fail(
        user,
        &format!("{}: log in failed: {}", shop.my_account, reason),
        &mut request,
        Some(&headers),
        Some(&html),
    )
}

/// Load My Account's orders list, for a logged in customer.
///
/// Fails when the page is not the logged in My Account (a user who is not
/// logged in gets the log in form instead). Returns the page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{my_account_orders, Shop};
///
/// transaction!(orders);
///
/// async fn orders(user: &mut GooseUser) -> TransactionResult {
///     let _orders = my_account_orders(user, &Shop::default()).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::orders_site("my-account/orders/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(orders(&mut user));
/// # assert!(result.is_ok(), "doc example my_account_orders: {}", support::failure(&result));
/// ```
pub async fn my_account_orders(
    user: &mut GooseUser,
    shop: &Shop<'_>,
) -> Result<String, Box<TransactionError>> {
    let account = crate::Validate::builder()
        .text("woocommerce-MyAccount-navigation")
        .build();
    let goose = user.get(shop.orders).await?;
    load_page(user, goose, Some(&account)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pages captured from the WooCommerce demo shop by
    // examples/wordpress-sites/capture-fixtures.sh.
    const PRODUCTS: &str = include_str!("../tests/fixtures/woocommerce/store-products.json");
    const CLASSIC_ADD: &str =
        include_str!("../tests/fixtures/woocommerce/classic-add-to-cart.html");
    const CLASSIC_ADD_INVALID: &str =
        include_str!("../tests/fixtures/woocommerce/classic-add-to-cart-invalid.html");
    const CLASSIC_UPDATED: &str =
        include_str!("../tests/fixtures/woocommerce/classic-cart-updated.html");
    const CHECKOUT_FAILURE: &str =
        include_str!("../tests/fixtures/woocommerce/classic-checkout-failure.json");
    const MY_ACCOUNT_FAILED: &str =
        include_str!("../tests/fixtures/woocommerce/my-account-login-failed.html");

    fn product(id: u64, purchasable: bool, in_stock: bool, has_options: bool) -> Product {
        Product {
            id,
            name: format!("Product {}", id),
            url: format!("/product/{}/", id),
            kind: "simple".to_string(),
            purchasable,
            in_stock,
            has_options,
        }
    }

    #[test]
    fn parse_product_cases() {
        let listed: serde_json::Value = serde_json::from_str(PRODUCTS).unwrap();
        let products: Vec<Product> = listed
            .as_array()
            .unwrap()
            .iter()
            .filter_map(parse_product)
            .collect();
        assert_eq!(products.len(), 5, "parse_product case count");
        assert_eq!(
            products[2],
            Product {
                id: 31,
                name: "Beanie with Logo".to_string(),
                url: "/product/beanie-with-logo/".to_string(),
                kind: "simple".to_string(),
                purchasable: true,
                in_stock: true,
                has_options: false,
            },
            "parse_product case simple product"
        );
        assert_eq!(products[0].kind, "external", "parse_product case external");
        assert!(
            !products[0].purchasable,
            "parse_product case external not purchasable"
        );
        assert_eq!(
            parse_product(&serde_json::json!({"id": 1, "permalink": "https://e.com/p/"}))
                .map(|p| (p.url, p.purchasable)),
            Some(("/p/".to_string(), false)),
            "parse_product case sparse"
        );
        assert_eq!(
            parse_product(&serde_json::json!({"id": 1})),
            None,
            "parse_product case no permalink"
        );
        assert_eq!(
            parse_product(&serde_json::json!({"permalink": "/p/"})),
            None,
            "parse_product case no id"
        );
    }

    #[test]
    fn can_add_to_cart_cases() {
        // (case, product, expected)
        let cases = [
            ("simple", product(1, true, true, false), true),
            ("not purchasable", product(1, false, true, false), false),
            ("out of stock", product(1, true, false, false), false),
            ("options", product(1, true, true, true), false),
        ];
        for (case, product, expected) in cases {
            assert_eq!(
                product.can_add_to_cart(),
                expected,
                "can_add_to_cart case {}",
                case
            );
        }
    }

    #[test]
    fn random_product_cases() {
        let products = [
            product(1, true, true, false),
            product(2, false, true, false),
            product(3, true, true, false),
            product(4, true, true, true),
        ];
        let mut picked = std::collections::HashSet::new();
        for _ in 0..200 {
            picked.insert(random_product(&products).unwrap().id);
        }
        assert_eq!(
            picked,
            vec![1, 3].into_iter().collect(),
            "random_product case only and every product that can go in the cart"
        );
        assert_eq!(
            random_product(&products[1..2]),
            None,
            "random_product case none"
        );
    }

    #[test]
    fn notice_cases() {
        assert!(has_success_notice(CLASSIC_ADD), "notice case added to cart");
        assert_eq!(
            error_notice(CLASSIC_ADD),
            None,
            "notice case added to cart has no error"
        );
        assert!(
            has_success_notice(CLASSIC_UPDATED),
            "notice case cart updated"
        );
        assert!(
            !has_success_notice(CLASSIC_ADD_INVALID),
            "notice case nothing added"
        );
        assert!(
            has_success_notice(r#"<div class="woocommerce-message" role="alert">Added</div>"#),
            "notice case classic theme success"
        );
        let failed = error_notice(MY_ACCOUNT_FAILED).unwrap();
        assert!(
            failed.starts_with("Error:") && failed.contains("password"),
            "notice case my account log in failed: {}",
            failed
        );
        let messages: serde_json::Value = serde_json::from_str(CHECKOUT_FAILURE).unwrap();
        let checkout = error_notice(messages["messages"].as_str().unwrap()).unwrap();
        assert!(
            checkout.contains("Billing Last name is a required field"),
            "notice case checkout failure: {}",
            checkout
        );
        assert_eq!(
            error_notice(
                r#"<ul class="woocommerce-error" role="alert"><li>Out of stock</li></ul>"#
            )
            .as_deref(),
            Some("Out of stock"),
            "notice case classic theme error"
        );
    }

    #[test]
    fn shop_builder_defaults() {
        let shop = Shop::builder().build();
        assert_eq!(shop.shop, "shop/", "Shop default shop");
        assert_eq!(shop.cart, "cart/", "Shop default cart");
        assert_eq!(shop.checkout, "checkout/", "Shop default checkout");
        assert_eq!(shop.my_account, "my-account/", "Shop default my_account");
        assert_eq!(shop.orders, "my-account/orders/", "Shop default orders");
        assert_eq!(
            shop.store_api, "wp-json/wc/store/v1/",
            "Shop default store_api"
        );
        assert_eq!(
            Shop::default().store_api,
            shop.store_api,
            "Shop default is the builder's"
        );
    }

    #[test]
    fn checkout_builder_defaults() {
        let checkout = Checkout::builder().build();
        assert_eq!(
            checkout.billing,
            [
                ("first_name", "Goose"),
                ("last_name", "Shopper"),
                ("address_1", "1 Main Street"),
                ("city", "San Francisco"),
                ("state", "CA"),
                ("postcode", "94105"),
                ("country", "US"),
                ("email", "goose@example.com"),
                ("phone", "5555550100"),
            ],
            "Checkout default billing"
        );
        assert_eq!(
            checkout.payment_method, "cod",
            "Checkout default payment_method"
        );
        let checkout = Checkout::builder()
            .billing("city", "Austin")
            .billing("company", "Tag1")
            .build();
        assert_eq!(
            checkout.billing.len(),
            10,
            "Checkout billing replaces or adds"
        );
        assert_eq!(
            checkout.billing("city"),
            Some("Austin"),
            "Checkout billing replaced"
        );
        assert_eq!(
            checkout.billing("company"),
            Some("Tag1"),
            "Checkout billing added"
        );
    }

    #[test]
    fn store_api_cart_defaults() {
        let shop = Shop::builder().store_api("api/").build();
        let cart = StoreApiCart::new(&shop);
        assert_eq!(
            cart.store_api, "api/",
            "StoreApiCart store_api from the shop"
        );
        assert_eq!(cart.nonce(), None, "StoreApiCart default nonce");
        assert_eq!(cart.cart_token(), None, "StoreApiCart default cart token");
        assert!(!cart.use_cart_token, "StoreApiCart default use_cart_token");
        assert!(cart.items().is_empty(), "StoreApiCart default items");
        assert!(
            cart.use_cart_token(true).use_cart_token,
            "StoreApiCart use_cart_token"
        );
    }
}
