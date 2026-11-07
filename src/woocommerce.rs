//! Functionality that's specific to WooCommerce.
//!
//! The helpers here drive any WooCommerce shop the way a shopper's browser does:
//! browsing its shop, products and search, adding to the cart from a product
//! page or the shop page, the block cart and checkout, the classic shortcode
//! cart and checkout, and My Account. Each loads the pages a browser loads, with
//! their static assets, and sends only the background requests those pages'
//! own scripts send, with the nonces the pages carry; what a browser requests
//! for each action is recorded under `tests/fixtures/browser/` in the
//! repository. They build on [`crate::wordpress`]. None of them assume a
//! product, a theme or a page path beyond WooCommerce's defaults, which
//! [`ShopBuilder`] changes.
//!
//! [`StoreApiCart`] is the exception: it drives the Store API directly, as a
//! headless storefront does, for load testing one on purpose.

use crate::wordpress::{
    attribute, fail, form_controls, get_form, get_form_by_class, get_named, get_rest_nonce,
    header_value, json_response, load_page, pagination_links, plain_text, query_encode,
    read_response, regex, request_ok, send_json, site_path,
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

    /// The cart page: the block cart [`cart_page`] loads, or the classic cart
    /// the classic flows use.
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

    /// The checkout page: the block checkout [`checkout_page`] loads, or the
    /// classic checkout [`classic_checkout`] uses.
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

    /// The Store API root, where the block cart and checkout send their
    /// requests, and [`StoreApiCart`] sends its own.
    ///
    /// Defaults to `wp-json/wc/store/v1/`.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::woocommerce::Shop;
    ///
    /// let _shop = Shop::builder().store_api("api/wc/store/v1/").build();
    /// # mod support { include!("../tests/doc/woocommerce.rs"); }
    /// # let server = support::cart_page_site_at("cart/", "api/wc/store/v1/");
    /// # support::update_cart_item("ShopBuilder::store_api", &server, &_shop);
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

// A product, as its card on the shop page shows it (see [`discover_products`]).
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

/// Find a shop's products as a shopper browsing it does: load the shop page and
/// each page its pagination links to, and read every product card on them, up
/// to `max` products.
///
/// A card is the `<li>` WooCommerce writes for a product, in a block theme's
/// product collection or a classic theme's product loop. Each [`Product`] is
/// read from its card: the ID from its `post-<id>` class, the page and name
/// from its link and title, the type from its `product-type-<type>` class,
/// whether it is purchasable and in stock from its `purchasable` and
/// `instock` (or `onbackorder`) classes, and whether it needs a choice first
/// from its button: a card whose button adds to the cart directly carries
/// `ajax_add_to_cart`; one that needs options links to the product instead.
///
/// The pages are loaded without their assets, each named `discover products`.
/// Fails when the shop page cannot be loaded, or shows no product cards (the
/// shop is empty, or is not at [`ShopBuilder::shop`]). Run it once per load test and
/// share the result, as with [`crate::wordpress::discover`].
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
/// # let server = support::shop_site("shop/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(find_products(&mut user));
/// # assert!(result.is_ok(), "doc example discover_products: {}", support::failure(&result));
/// ```
pub async fn discover_products(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    max: usize,
) -> Result<Vec<Product>, Box<TransactionError>> {
    let mut products: Vec<Product> = Vec::new();
    let start = site_path(&user.build_url(shop.shop)?);
    let mut queue = std::collections::VecDeque::from([start.clone()]);
    let mut seen = std::collections::HashSet::from([start]);
    let mut first: Option<(GooseRequestMetric, String)> = None;
    while let Some(path) = queue.pop_front() {
        if products.len() >= max {
            break;
        }
        let goose = get_named(user, &path, "discover products").await?;
        request_ok(&goose)?;
        let request = goose.request.clone();
        let page_url = goose.request.final_url.to_string();
        let (_, html) = read_response(goose).await;
        if first.is_none() {
            first = Some((request, html.clone()));
        }
        for product in product_cards(&html) {
            if !products.iter().any(|p| p.id == product.id) {
                products.push(product);
            }
        }
        for link in pagination_links(user, &page_url, &html) {
            if seen.insert(link.clone()) {
                queue.push_back(link);
            }
        }
    }
    if products.is_empty() {
        if let Some((mut request, html)) = first {
            return fail(
                user,
                &format!("{}: no product cards on the shop page", shop.shop),
                &mut request,
                None,
                Some(&html),
            );
        }
    }
    products.truncate(max);
    Ok(products)
}

// The products on a page, one from each product card, in page order.
pub(crate) fn product_cards(html: &str) -> Vec<Product> {
    let item = regex(r"(?is)<li\b[^>]*>");
    let starts: Vec<(usize, String)> = item
        .find_iter(html)
        .filter_map(|m| {
            let class = attribute(m.as_str(), "class")?;
            class
                .split_whitespace()
                .any(|c| c == "type-product")
                .then(|| (m.start(), class))
        })
        .collect();
    let post = regex(r"^post-(\d+)$");
    let heading = regex(r"(?is)<h[1-6]\b[^>]*>(.*?)</h[1-6]\s*>");
    let mut products = Vec::new();
    for (n, (start, class)) in starts.iter().enumerate() {
        let end = starts.get(n + 1).map_or(html.len(), |(next, _)| *next);
        let card = &html[*start..end];
        let card = match card.find("</ul") {
            Some(list_end) => &card[..list_end],
            None => card,
        };
        let classes: Vec<&str> = class.split_whitespace().collect();
        let has = |name: &str| classes.contains(&name);
        let id = classes
            .iter()
            .find_map(|c| post.captures(c).and_then(|c| c[1].parse::<u64>().ok()));
        let url = tag_regex_a()
            .find_iter(card)
            .find_map(|a| attribute(a.as_str(), "href"));
        let (id, url) = match (id, url) {
            (Some(id), Some(url)) => (id, site_path(&url)),
            _ => continue,
        };
        let name = heading
            .captures(card)
            .map(|c| plain_text(&c[1]))
            .unwrap_or_default();
        let kind = classes
            .iter()
            .find_map(|c| c.strip_prefix("product-type-"))
            .unwrap_or_default()
            .to_string();
        let purchasable = has("purchasable");
        let in_stock = has("instock") || has("onbackorder");
        let direct = tag_regex_a()
            .find_iter(card)
            .chain(button_regex().find_iter(card))
            .filter_map(|tag| attribute(tag.as_str(), "class"))
            .any(|class| class.split_whitespace().any(|c| c == "ajax_add_to_cart"));
        products.push(Product {
            id,
            name,
            url,
            kind,
            purchasable,
            in_stock,
            has_options: purchasable && in_stock && !direct,
        });
    }
    products
}

fn tag_regex_a() -> Regex {
    regex(r"(?is)<a\b[^>]*>")
}

fn button_regex() -> Regex {
    regex(r"(?is)<button\b[^>]*>")
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

/// A user's cart driven straight through the Store API, as a headless storefront
/// (a separate front end on WooCommerce's API) drives it.
///
/// This is not what a browser on a standard theme does. A shopper's browser
/// loads the product page, the cart page and the checkout page, and the Store
/// API requests it sends are the few those pages' scripts send, with the nonce
/// the page carries: [`add_to_cart`], [`cart_page`], [`update_cart_item`],
/// [`remove_cart_item`], [`checkout_page`] and [`place_order`] do that. Use
/// `StoreApiCart` only to load test a headless storefront's use of the API on
/// purpose; it skips every page a browser on the shop would load.
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

/// Used to build a [`Checkout`], which [`place_order`], [`classic_checkout`] and
/// [`StoreApiCart::checkout`] take.
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
    let re = regex(
        r#"(?s)class="[^"]*(?:woocommerce-error|is-error)[^"]*"[^>]*>(.*?)</(?:ul|div)>\s*(?:</div>)?"#,
    );
    re.captures(html).map(|c| plain_text(&c[1]))
}

// Whether a page shows a success notice, classic or block themed.
fn has_success_notice(html: &str) -> bool {
    regex(r#"class="[^"]*(?:woocommerce-message|is-success)[^"]*""#).is_match(html)
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
    let quantity_field = regex(r"^cart\[[^\]]+\]\[qty\]$");
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
        crate::page::load_assets(user, &request.final_url, &html).await;
        crate::page::background_requests(user, &html).await?;
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

// The form WooCommerce writes on a product page for adding it to the cart,
// `<form class="cart">`: its action and contents.
fn add_to_cart_form(html: &str) -> Option<(String, String)> {
    let end = regex(r"(?i)</form\s*>");
    regex(r"(?is)<form\b[^>]*>")
        .find_iter(html)
        .find(|form| {
            attribute(form.as_str(), "class")
                .is_some_and(|c| c.split_whitespace().any(|c| c == "cart"))
        })
        .map(|form| {
            let action = attribute(form.as_str(), "action").unwrap_or_default();
            let rest = &html[form.end()..];
            let contents = end.find(rest).map_or(rest, |close| &rest[..close.start()]);
            (action, contents.to_string())
        })
}

/// Add a product to the cart from its page, as a shopper does: load the
/// product's page (with its assets and the Store API cart read its scripts
/// make), then submit its add to cart form, `<form class="cart">`, with
/// `quantity` and the Add to cart button's `add-to-cart` value, to the form's
/// action, the product's own URL. WooCommerce answers with the product page
/// again, showing an "added to your cart" notice; it is loaded the same way.
///
/// Fails when the product page has no add to cart form (the product is out of
/// stock, or not sold), the form needs a choice first (a variable product's
/// `variations_form`), or WooCommerce shows an error notice instead of the
/// success notice, which the failure names (for example not enough stock).
/// The requests are named `product page` and `add to cart`, so every product
/// shares their lines in the metrics. Returns the page WooCommerce answered with.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{add_to_cart, Product};
///
/// transaction!(add);
///
/// async fn add(user: &mut GooseUser) -> TransactionResult {
///     // A product as discover_products reads it from the shop page.
///     let beanie = Product {
///         id: 14,
///         name: "Beanie".to_string(),
///         url: "/product/beanie/".to_string(),
///         kind: "simple".to_string(),
///         purchasable: true,
///         in_stock: true,
///         has_options: false,
///     };
///     let _page = add_to_cart(user, &beanie, 2).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::add_to_cart_site("/product/beanie/", 14, 2);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(add(&mut user));
/// # assert!(result.is_ok(), "doc example add_to_cart: {}", support::failure(&result));
/// # assert_eq!(support::adds(&server), 1, "doc example add_to_cart: forms submitted");
/// ```
pub async fn add_to_cart(
    user: &mut GooseUser,
    product: &Product,
    quantity: u64,
) -> Result<String, Box<TransactionError>> {
    let validate = crate::Validate::builder().text("single-product").build();
    let goose = get_named(user, &product.url, "product page").await?;
    let mut request = goose.request.clone();
    let page = load_page(user, goose, Some(&validate)).await?;
    let (action, form) = match add_to_cart_form(&page) {
        Some(form) => form,
        None => {
            let stock = regex(r#"(?is)<p\b[^>]*class="[^"]*\bstock\b[^"]*"[^>]*>(.*?)</p>"#)
                .captures(&page)
                .map(|c| plain_text(&c[1]))
                .unwrap_or_else(|| "not sold".to_string());
            return fail(
                user,
                &format!("{}: no add to cart form on page: {}", product.url, stock),
                &mut request,
                None,
                Some(&page),
            );
        }
    };
    let button = regex(r"(?is)<button\b[^>]*>")
        .find_iter(&form)
        .map(|b| b.as_str().to_string())
        .find(|b| attribute(b, "name").as_deref() == Some("add-to-cart"));
    let value = match button.and_then(|b| attribute(&b, "value")) {
        Some(value) if !form.contains("variation_id") => value,
        _ => {
            return fail(
                user,
                &format!(
                    "{}: the add to cart form needs a choice first (a variable product?)",
                    product.url
                ),
                &mut request,
                None,
                Some(&page),
            )
        }
    };
    let mut params: Vec<(String, String)> = form_controls(&form)
        .into_iter()
        .map(|(name, value)| {
            if name == "quantity" {
                (name, quantity.to_string())
            } else {
                (name, value)
            }
        })
        .collect();
    params.push(("add-to-cart".to_string(), value));
    let action = if action.is_empty() {
        product.url.clone()
    } else {
        site_path(&action)
    };
    let builder = user
        .get_request_builder(&GooseMethod::Post, &action)?
        .form(&params);
    let request = GooseRequest::builder()
        .method(GooseMethod::Post)
        .path(action.as_str())
        .name("add to cart")
        .set_request_builder(builder)
        .build();
    let goose = user.request(request).await?;
    expect_success_notice(user, goose, "not added to cart").await
}

/// Add a product to the cart from a product archive (the shop page, or a
/// category's), as a shopper does with the Add to cart button on its card:
/// load the archive (with its assets and the Store API cart read its scripts
/// make, whose `Nonce` header the button sends back), then send what the
/// button's script sends, one `cart/add-item` of one product in a Store API
/// `batch` POST.
///
/// Fails when the product has no Add to cart button on the archive (it is not
/// on that page, or needs a choice first), the archive's cart read gave no
/// nonce, or WooCommerce refuses the item, naming its error. The requests are
/// named `product archive` and `<store_api>batch`. Returns the cart, as JSON.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{add_to_cart_from_archive, Product, Shop};
///
/// transaction!(add);
///
/// async fn add(user: &mut GooseUser) -> TransactionResult {
///     let hoodie = Product {
///         id: 12,
///         name: "Hoodie with Logo".to_string(),
///         url: "/product/hoodie-with-logo/".to_string(),
///         kind: "simple".to_string(),
///         purchasable: true,
///         in_stock: true,
///         has_options: false,
///     };
///     let cart = add_to_cart_from_archive(user, &Shop::default(), "shop/", &hoodie).await?;
///     assert!(cart["items"].is_array());
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::archive_add_site("shop/", 12);
/// # let mut user = support::user(&server);
/// # let result = support::block_on(add(&mut user));
/// # assert!(result.is_ok(), "doc example add_to_cart_from_archive: {}", support::failure(&result));
/// # assert_eq!(support::adds(&server), 1, "doc example add_to_cart_from_archive: items added");
/// ```
pub async fn add_to_cart_from_archive(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    archive: &str,
    product: &Product,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let goose = get_named(user, archive, "product archive").await?;
    let mut request = goose.request.clone();
    let page = crate::page::open(user, goose, None).await?;
    let button = format!(r#"data-product_id="{}""#, product.id);
    let direct = product_cards(&page.html)
        .iter()
        .any(|p| p.id == product.id && !p.has_options)
        && page.html.contains(&button);
    let nonce = match (direct, page.store_api_nonce) {
        (true, Some(nonce)) => nonce,
        (false, _) => {
            return fail(
                user,
                &format!(
                    "{}: product {} has no Add to cart button on the page",
                    archive, product.id
                ),
                &mut request,
                None,
                Some(&page.html),
            )
        }
        (true, None) => {
            return fail(
                user,
                &format!("{}: the page's cart read gave no Store API nonce", archive),
                &mut request,
                None,
                Some(&page.html),
            )
        }
    };
    let body = serde_json::json!({ "requests": [{
        "path": "/wc/store/v1/cart/add-item",
        "method": "POST",
        "headers": { "Nonce": nonce, "Content-Type": "application/json" },
        "body": { "id": product.id, "quantity": 1 },
    }]});
    let path = format!("{}batch", shop.store_api);
    let goose = send_json(
        user,
        GooseMethod::Post,
        &path,
        &path,
        &[("Nonce", nonce.as_str())],
        Some(&body),
    )
    .await?;
    batch_response(user, goose, &path).await
}

// The one response in a Store API batch answer, failing when the batch or the
// request in it failed; WooCommerce's error code and message name the failure.
async fn batch_response(
    user: &mut GooseUser,
    goose: goose::goose::GooseResponse,
    path: &str,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let mut request = goose.request.clone();
    let (headers, json) = json_response(user, goose).await?;
    let response = &json["responses"][0];
    let status = response["status"].as_u64().unwrap_or(0);
    if (200..300).contains(&status) {
        return Ok(response["body"].clone());
    }
    fail(
        user,
        &format!(
            "{}: {} {}",
            path,
            status,
            crate::wordpress::rest_error(&response["body"])
        ),
        &mut request,
        Some(&headers),
        Some(&json.to_string()),
    )
}

/// Load the cart page, as a shopper viewing the cart does: the page and its
/// assets. WooCommerce's block cart preloads the cart into the page, so its
/// scripts make no request on opening.
///
/// The request is named `cart page`. Returns the page, which
/// [`update_cart_item`] and [`remove_cart_item`] act on.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{cart_page, Shop};
///
/// transaction!(view_cart);
///
/// async fn view_cart(user: &mut GooseUser) -> TransactionResult {
///     let _cart = cart_page(user, &Shop::default()).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::cart_page_site("cart/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(view_cart(&mut user));
/// # assert!(result.is_ok(), "doc example cart_page: {}", support::failure(&result));
/// ```
pub async fn cart_page(
    user: &mut GooseUser,
    shop: &Shop<'_>,
) -> Result<String, Box<TransactionError>> {
    let validate = crate::Validate::builder().text("woocommerce-cart").build();
    let goose = get_named(user, shop.cart, "cart page").await?;
    load_page(user, goose, Some(&validate)).await
}

// What a block cart or checkout page gives its scripts: the Store API nonce,
// the REST nonce and the cart it preloads.
struct StorePage {
    nonce: String,
    rest_nonce: Option<String>,
    cart: serde_json::Value,
}

// Read the Store API nonce and the preloaded cart from a block cart or checkout
// page, failing on `request` when the page has neither.
fn store_page(html: &str) -> Result<StorePage, String> {
    let nonce = regex(r#"storeApiNonce\s*:\s*['"]([^'"]+)['"]"#)
        .captures(html)
        .map(|c| c[1].to_string());
    let cart = crate::page::preloaded(html)
        .get("/wc/store/v1/cart")
        .map(|cart| cart["body"].clone());
    match (nonce, cart) {
        (Some(nonce), Some(cart)) => Ok(StorePage {
            nonce,
            rest_nonce: get_rest_nonce(html),
            cart,
        }),
        (None, _) => Err("no Store API nonce on the page".to_string()),
        (_, None) => Err("no preloaded cart on the page".to_string()),
    }
}

// The headers the block cart and checkout send with every Store API request.
fn store_headers(page: &StorePage) -> Vec<(&str, &str)> {
    let mut headers = vec![("Nonce", page.nonce.as_str())];
    if let Some(rest_nonce) = page.rest_nonce.as_deref() {
        headers.push(("X-WP-Nonce", rest_nonce));
    }
    headers
}

// The cart line holding `product_id`, from a cart as the Store API returns it.
fn cart_line(cart: &serde_json::Value, product_id: u64) -> Option<String> {
    cart["items"]
        .as_array()?
        .iter()
        .find(|item| item["id"].as_u64() == Some(product_id))
        .and_then(|item| item["key"].as_str())
        .map(str::to_string)
}

// Fail the transaction on a page that cannot do what was asked of it. The
// page's request was made earlier, by another call, so no request is failed:
// the transaction fails with the reason, which the debug log also gets.
fn page_fail<T>(
    user: &mut GooseUser,
    page_path: &str,
    reason: &str,
    html: &str,
) -> Result<T, Box<TransactionError>> {
    let reason = format!("{}: {}", page_path, reason);
    log::warn!("{}", reason);
    user.log_debug(&reason, None, None, Some(html))?;
    Err(Box::new(TransactionError::Custom(reason)))
}

/// Change the quantity of a product's line on a loaded cart page, as the cart's
/// own quantity controls do: a Store API `cart/update-item` POST, `?_locale=site`,
/// with the line's key from the cart the page preloads and the page's Store API
/// and REST nonces.
///
/// `cart` is the page [`cart_page`] returned. Fails when the page has no Store
/// API nonce or preloaded cart (it is not WooCommerce's block cart), the
/// product is not in the cart, or WooCommerce refuses the change, naming its
/// error. The request is named `<store_api>cart/update-item`. Returns the
/// cart, as JSON.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{cart_page, update_cart_item, Shop};
///
/// transaction!(buy_more);
///
/// async fn buy_more(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::default();
///     let cart = cart_page(user, &shop).await?;
///     let updated = update_cart_item(user, &shop, &cart, 14, 2).await?;
///     assert_eq!(updated["items"][0]["quantity"], 2);
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::cart_page_site("cart/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(buy_more(&mut user));
/// # assert!(result.is_ok(), "doc example update_cart_item: {}", support::failure(&result));
/// ```
pub async fn update_cart_item(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    cart: &str,
    product_id: u64,
    quantity: u64,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let page = match store_page(cart) {
        Ok(page) => page,
        Err(reason) => return page_fail(user, shop.cart, &reason, cart),
    };
    let key = match cart_line(&page.cart, product_id) {
        Some(key) => key,
        None => {
            let reason = format!("product {} is not in the cart", product_id);
            return page_fail(user, shop.cart, &reason, cart);
        }
    };
    let path = format!("{}cart/update-item?_locale=site", shop.store_api);
    let name = format!("{}cart/update-item", shop.store_api);
    let body = serde_json::json!({ "key": key, "quantity": quantity });
    let goose = send_json(
        user,
        GooseMethod::Post,
        &path,
        &name,
        &store_headers(&page),
        Some(&body),
    )
    .await?;
    json_response(user, goose).await.map(|(_, json)| json)
}

/// Remove a product's line on a loaded cart page, as the cart's own remove
/// control does: a Store API `batch` POST, `?_locale=site`, holding one
/// `cart/remove-item` with the line's key, and the page's nonces.
///
/// `cart` is the page [`cart_page`] returned. Fails as [`update_cart_item`]
/// does. The request is named `<store_api>batch`. Returns the cart, as JSON.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{cart_page, remove_cart_item, Shop};
///
/// transaction!(change_mind);
///
/// async fn change_mind(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::default();
///     let cart = cart_page(user, &shop).await?;
///     let _cart = remove_cart_item(user, &shop, &cart, 14).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::cart_page_site("cart/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(change_mind(&mut user));
/// # assert!(result.is_ok(), "doc example remove_cart_item: {}", support::failure(&result));
/// ```
pub async fn remove_cart_item(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    cart: &str,
    product_id: u64,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let page = match store_page(cart) {
        Ok(page) => page,
        Err(reason) => return page_fail(user, shop.cart, &reason, cart),
    };
    let key = match cart_line(&page.cart, product_id) {
        Some(key) => key,
        None => {
            let reason = format!("product {} is not in the cart", product_id);
            return page_fail(user, shop.cart, &reason, cart);
        }
    };
    let body = serde_json::json!({ "requests": [{
        "path": "/wc/store/v1/cart/remove-item",
        "data": { "key": key },
        "method": "POST",
        "cache": "no-store",
        "body": { "key": key },
        "headers": { "Nonce": page.nonce },
    }]});
    store_batch(user, shop, &page, &body).await
}

// Send a Store API batch from a block cart or checkout page, as its scripts do.
async fn store_batch(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    page: &StorePage,
    body: &serde_json::Value,
) -> Result<serde_json::Value, Box<TransactionError>> {
    let path = format!("{}batch?_locale=site", shop.store_api);
    let name = format!("{}batch", shop.store_api);
    let goose = send_json(
        user,
        GooseMethod::Post,
        &path,
        &name,
        &store_headers(page),
        Some(body),
    )
    .await?;
    batch_response(user, goose, &name).await
}

/// Load the checkout page, as a shopper going to checkout does: the page and
/// its assets. WooCommerce's block checkout preloads the cart and the
/// customer's addresses into the page, so its scripts make no request on
/// opening.
///
/// The request is named `checkout page`. Returns the page, which
/// [`place_order`] acts on.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{checkout_page, Shop};
///
/// transaction!(go_to_checkout);
///
/// async fn go_to_checkout(user: &mut GooseUser) -> TransactionResult {
///     let _checkout = checkout_page(user, &Shop::default()).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::checkout_page_site("checkout/");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(go_to_checkout(&mut user));
/// # assert!(result.is_ok(), "doc example checkout_page: {}", support::failure(&result));
/// ```
pub async fn checkout_page(
    user: &mut GooseUser,
    shop: &Shop<'_>,
) -> Result<String, Box<TransactionError>> {
    let validate = crate::Validate::builder()
        .text("woocommerce-checkout")
        .build();
    let goose = get_named(user, shop.checkout, "checkout page").await?;
    load_page(user, goose, Some(&validate)).await
}

// The order number an order received page shows: the block theme's order
// summary ("Order #:") or the classic overview ("Order number:").
pub(crate) fn order_number(html: &str) -> Option<String> {
    let patterns = [
        r#"(?s)order-confirmation-summary-list-item__key">[^<]*</span>\s*<span[^>]*__value">\s*([^<\s]+)\s*<"#,
        r#"(?s)woocommerce-order-overview__order[^"]*"[^>]*>.*?<strong>\s*([^<\s]+)\s*</strong>"#,
    ];
    patterns.iter().find_map(|pattern| {
        regex(pattern)
            .captures(html)
            .map(|c| plain_text(&c[1]))
            .filter(|n| !n.is_empty())
    })
}

/// Place the order on a loaded checkout page, as a shopper does on
/// WooCommerce's block checkout: type in the address from `checkout` and click
/// Place order.
///
/// The checkout's scripts save each change to the customer as it is typed, as
/// a Store API `batch` POST holding a `cart/update-customer`: one when the
/// email address differs from what the page preloaded, one when the rest of
/// the address does (the shipping address is the billing address). A
/// returning customer whose address is on file sends neither. Place order is
/// then a Store API `checkout` POST of the addresses, the payment method and
/// the cart's total, every request with the page's Store API and REST nonces
/// and `?_locale=site`. The order received page WooCommerce sends the browser
/// to is then loaded, with its assets.
///
/// `checkout_page` is the page [`checkout_page`] returned. Fails when the page
/// has no Store API nonce or preloaded cart, the cart is empty, WooCommerce
/// refuses the checkout (a 400, such as `woocommerce_rest_invalid_address`, or
/// a payment that did not succeed), or the order received page shows no order
/// number. The requests are named `<store_api>batch`, `<store_api>checkout` and
/// `order received`. Returns the order received page.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::woocommerce::{checkout_page, place_order, Checkout, Shop};
///
/// transaction!(buy);
///
/// async fn buy(user: &mut GooseUser) -> TransactionResult {
///     let shop = Shop::default();
///     let page = checkout_page(user, &shop).await?;
///     let _received = place_order(user, &shop, &page, &Checkout::builder().build()).await?;
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/woocommerce.rs"); }
/// # let server = support::place_order_site();
/// # let mut user = support::user(&server);
/// # let result = support::block_on(buy(&mut user));
/// # assert!(result.is_ok(), "doc example place_order: {}", support::failure(&result));
/// # assert_eq!(support::checkouts(&server), 1, "doc example place_order: orders placed");
/// ```
pub async fn place_order(
    user: &mut GooseUser,
    shop: &Shop<'_>,
    checkout_page: &str,
    checkout: &Checkout<'_>,
) -> Result<String, Box<TransactionError>> {
    let page = match store_page(checkout_page) {
        Ok(page) => page,
        Err(reason) => return page_fail(user, shop.checkout, &reason, checkout_page),
    };
    if page.cart["items"]
        .as_array()
        .is_none_or(|items| items.is_empty())
    {
        return page_fail(user, shop.checkout, "the cart is empty", checkout_page);
    }

    // The address as the page has it, and as the shopper types it in.
    let on_file = page.cart["billing_address"].clone();
    let field = |address: &serde_json::Value, name: &str| {
        address[name].as_str().unwrap_or_default().to_string()
    };
    let mut billing = serde_json::Map::new();
    for name in [
        "first_name",
        "last_name",
        "company",
        "address_1",
        "address_2",
        "city",
        "state",
        "postcode",
        "country",
        "email",
        "phone",
    ] {
        let value = checkout
            .billing(name)
            .map(str::to_string)
            .unwrap_or_else(|| field(&on_file, name));
        billing.insert(name.to_string(), value.into());
    }
    let billing = serde_json::Value::Object(billing);
    let mut shipping = billing.clone();
    if let Some(shipping) = shipping.as_object_mut() {
        shipping.remove("email");
    }

    let update = |data: serde_json::Value| {
        serde_json::json!({ "requests": [{
            "path": "/wc/store/v1/cart/update-customer",
            "method": "POST",
            "data": data,
            "cache": "no-store",
            "body": data,
            "headers": { "Nonce": page.nonce },
        }]})
    };
    if field(&billing, "email") != field(&on_file, "email") {
        let mut typed = on_file.clone();
        typed["email"] = billing["email"].clone();
        store_batch(
            user,
            shop,
            &page,
            &update(serde_json::json!({ "billing_address": typed })),
        )
        .await?;
    }
    let rest_differs = billing
        .as_object()
        .into_iter()
        .flatten()
        .any(|(name, value)| name != "email" && value.as_str() != on_file[name].as_str());
    if rest_differs {
        store_batch(
            user,
            shop,
            &page,
            &update(serde_json::json!({
                "billing_address": billing,
                "shipping_address": shipping,
            })),
        )
        .await?;
    }

    let body = serde_json::json!({
        "additional_fields": {},
        "billing_address": billing,
        "create_account": false,
        "customer_note": "",
        "customer_password": "",
        "shipping_address": shipping,
        "expected_total": page.cart["totals"]["total_price"],
        "payment_method": checkout.payment_method,
        "payment_data": [{
            "key": format!("wc-{}-new-payment-method", checkout.payment_method),
            "value": false,
        }],
    });
    let path = format!("{}checkout?_locale=site", shop.store_api);
    let name = format!("{}checkout", shop.store_api);
    let goose = send_json(
        user,
        GooseMethod::Post,
        &path,
        &name,
        &store_headers(&page),
        Some(&body),
    )
    .await?;
    let mut request = goose.request.clone();
    let (headers, order) = json_response(user, goose).await?;
    let status = order["payment_result"]["payment_status"]
        .as_str()
        .unwrap_or("none");
    let redirect = order["payment_result"]["redirect_url"].as_str();
    let redirect = match redirect {
        Some(redirect)
            if order["order_id"].as_u64().is_some() && matches!(status, "success" | "pending") =>
        {
            site_path(redirect)
        }
        _ => {
            return fail(
                user,
                &format!("{}: order not placed: payment {}", name, status),
                &mut request,
                Some(&headers),
                Some(&order.to_string()),
            )
        }
    };
    let goose = get_named(user, &redirect, "order received").await?;
    let mut request = goose.request.clone();
    let html = load_page(user, goose, None).await?;
    if order_number(&html).is_none() {
        return fail(
            user,
            &format!("{}: no order number on the order received page", redirect),
            &mut request,
            None,
            Some(&html),
        );
    }
    Ok(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pages captured from the WooCommerce demo shop by
    // examples/wordpress-sites/capture-fixtures.sh.
    const SHOP: &str = include_str!("../tests/fixtures/woocommerce/shop-block.html");
    const SHOP_PAGE_2: &str = include_str!("../tests/fixtures/woocommerce/shop-block-page-2.html");
    const CLASSIC_SHOP: &str = include_str!("../tests/fixtures/woocommerce/shop.html");
    const ORDER_RECEIVED: &str =
        include_str!("../tests/fixtures/woocommerce/order-received-block.html");
    const ORDER_RECEIVED_CLASSIC: &str =
        include_str!("../tests/fixtures/woocommerce/order-received.html");
    const ORDER_RECEIVED_WRONG_KEY: &str =
        include_str!("../tests/fixtures/woocommerce/order-received-wrong-key.html");
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
    fn product_cards_cases() {
        let products = product_cards(SHOP);
        assert_eq!(
            products.len(),
            16,
            "product_cards case block shop page count"
        );
        let beanie = products.iter().find(|p| p.id == 14).expect("the beanie");
        assert_eq!(
            beanie,
            &Product {
                id: 14,
                name: "Beanie".to_string(),
                url: "/product/beanie/".to_string(),
                kind: "simple".to_string(),
                purchasable: true,
                in_stock: true,
                has_options: false,
            },
            "product_cards case simple product"
        );
        let vneck = products.iter().find(|p| p.id == 10).expect("the v-neck");
        assert_eq!(
            (vneck.kind.as_str(), vneck.has_options),
            ("variable", true),
            "product_cards case a card with options"
        );
        let external = product_cards(SHOP_PAGE_2)
            .into_iter()
            .find(|p| p.kind == "external")
            .expect("the external product");
        assert!(
            !external.purchasable && !external.has_options,
            "product_cards case external: {:?}",
            external
        );
        // A classic theme's product loop.
        let classic = product_cards(CLASSIC_SHOP);
        assert!(
            classic.iter().any(|p| p.can_add_to_cart()),
            "product_cards case classic loop: {:?}",
            classic
        );
        let card = r#"<ul><li class="product type-product post-7 outofstock product-type-simple"><a href="https://example.com/p/"><h2>Gone</h2></a></li></ul>"#;
        assert_eq!(
            product_cards(card)
                .first()
                .map(|p| (p.id, p.in_stock, p.has_options, p.url.as_str())),
            Some((7, false, false, "/p/")),
            "product_cards case out of stock"
        );
        assert!(
            product_cards("<ul><li class=\"menu-item\">x</li></ul>").is_empty(),
            "product_cards case no products"
        );
    }

    #[test]
    fn order_number_cases() {
        assert!(
            order_number(ORDER_RECEIVED).is_some_and(|n| n.parse::<u64>().is_ok()),
            "order_number case block order summary"
        );
        assert!(
            order_number(ORDER_RECEIVED_CLASSIC).is_some(),
            "order_number case classic overview"
        );
        assert_eq!(
            order_number(ORDER_RECEIVED_WRONG_KEY),
            None,
            "order_number case wrong order key"
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
