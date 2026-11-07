# What a browser requests

Recorded from headless Chromium by `scripts/browser-capture/capture.ts` on the two demo
sites under `examples/wordpress-sites/`, one HAR per action. These are the specification
the `wordpress` and `woocommerce` helpers follow, and the parity tests
(`tests/sites/parity.rs`) compare each helper's requests with them.

Each action starts where the shopper or editor already is (logged in, a product in the
cart, the page open), which the capture sets up without recording. Paths are on the
site; `{n}` marks a query value or path part that varies between runs. Type is
`document` (a page the browser navigates to, including form posts and redirects),
`asset` (a script, stylesheet, image or font) or `xhr` (a background request a script
sends, with the script that sent it).

## WordPress (theme test data site)

### home

Load the home page. File: `wordpress/home.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/` | document | navigation |

Assets: 52 (12 on the site), not listed row by row: 6 stylesheet, 4 script, 31 image, 9 media, 2 font.

### post

Load a post. File: `wordpress/post.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/hello-world/` | document | navigation |

Assets: 8 (6 on the site), not listed row by row: 1 stylesheet, 4 script, 2 image, 1 font.

### page

Load a page. File: `wordpress/page.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/about/` | document | navigation |

Assets: 4 (4 on the site), not listed row by row: 3 script, 1 font.

### category

Load a category archive. File: `wordpress/category.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/category/markup/` | document | navigation |

Assets: 12 (4 on the site), not listed row by row: 3 script, 8 image, 1 font.

### search

Search from the home page's search form. File: `wordpress/search.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/?s=post` | document | navigation |
| 8 | GET | `/embed/hrPKeL5t?hd=1&cover=1` | document | navigation |

Assets: 23 (5 on the site), not listed row by row: 5 script, 13 image, 2 font, 1 stylesheet, 2 media.

### comment

Post a comment as the logged in editor, from the post's comment form. File: `wordpress/comment.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/hello-world/` | document | navigation |
| 15 | POST | `/wp-comments-post.php` (302) | document | navigation |
| 16 | GET | `/hello-world/` | document | navigation |

Assets: 26 (20 on the site), not listed row by row: 6 stylesheet, 12 script, 6 image, 2 font.

### log-in

Log in as the editor through wp-login.php. File: `wordpress/log-in.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/wp-login.php` | document | navigation |
| 14 | POST | `/wp-login.php` (302) | document | navigation |
| 15 | GET | `/wp-admin/` | document | navigation |
| 97 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=1&_ajax_nonce=0e653d621f&1794039274580` | xhr | `/wp-admin/` |
| 99 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=2&_ajax_nonce=0e653d621f&1794039274596` | xhr | `/wp-admin/` |
| 100 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=no&_ajax_nonce=0e653d621f&1794039274603` | xhr | `/wp-admin/` |

Assets: 95 (90 on the site), not listed row by row: 5 stylesheet, 83 script, 7 image.

### log-out

Log out with the log out link on the dashboard. File: `wordpress/log-out.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/wp-admin/` | document | navigation |
| 83 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=1&_ajax_nonce=acce42fdf6&1794039280211` | xhr | `/wp-admin/` |
| 86 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=2&_ajax_nonce=acce42fdf6&1794039280219` | xhr | `/wp-admin/` |
| 87 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=no&_ajax_nonce=acce42fdf6&1794039280226` | xhr | `/wp-admin/` |
| 88 | GET | `/wp-login.php?action=logout&_wpnonce=bf4c99fae3` (302) | document | navigation |
| 89 | GET | `/wp-login.php?loggedout=true&wp_lang=en_US` | document | navigation |

Assets: 95 (90 on the site), not listed row by row: 5 stylesheet, 83 script, 7 image.

### create-post

Open the block editor on a new post, give it a title, and save the draft. File: `wordpress/create-post.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/wp-admin/post-new.php` | document | navigation |
| 138 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=1&_ajax_nonce=30830b9d40&1794039286528` | xhr | `/wp-admin/post-new.php` |
| 140 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=2&_ajax_nonce=30830b9d40&1794039286542` | xhr | `/wp-admin/post-new.php` |
| 141 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=no&_ajax_nonce=30830b9d40&1794039286574` | xhr | `/wp-admin/post-new.php` |
| 142 | GET | `/wp-json/wp/v2/comments?context=edit&post=1841&type=note&status=all&per_page=100&_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 151 | GET | `/wp-json/wp/v2/block-patterns/patterns?_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 152 | GET | `/wp-json/wp/v2/blocks?context=edit&per_page=100&page=1&_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 153 | GET | `/wp-json/wp/v2/wp_pattern_category?context=view&per_page=100&_fields=id%2Cname%2Cdescription%2Cslug&_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 154 | POST | `/wp-json/wp/v2/posts/1841?_locale=user` | xhr | `js/dist/api-fetch.min.js` |

Assets: 145 (144 on the site), not listed row by row: 11 stylesheet, 130 script, 2 image, 2 font.

### update-post

Open an existing post in the block editor, change its title, and save it. File: `wordpress/update-post.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/wp-admin/post.php?post=1&action=edit` | document | navigation |
| 138 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=1&_ajax_nonce=8b9411d8a9&1794039292958` | xhr | `/wp-admin/post.php` |
| 140 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=2&_ajax_nonce=8b9411d8a9&1794039292971` | xhr | `/wp-admin/post.php` |
| 141 | GET | `/wp-admin/admin-ajax.php?action=wp-compression-test&test=no&_ajax_nonce=8b9411d8a9&1794039293004` | xhr | `/wp-admin/post.php` |
| 142 | GET | `/wp-json/wp/v2/comments?context=edit&post=1&type=note&status=all&per_page=100&_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 151 | POST | `/wp-json/wp/v2/posts/1?_locale=user` | xhr | `js/dist/api-fetch.min.js` |
| 152 | POST | `/wp-admin/admin-ajax.php` | xhr | `/wp-admin/load-scripts.php` |

Assets: 145 (144 on the site), not listed row by row: 11 stylesheet, 130 script, 2 image, 2 font.

## WooCommerce (demo shop)

### shop

Load the shop page. File: `woocommerce/shop.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/shop/` | document | navigation |
| 30 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |

Assets: 29 (29 on the site), not listed row by row: 11 stylesheet, 16 script, 1 font, 1 image.

### product-category

Load a product category archive. File: `woocommerce/product-category.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/product-category/accessories/` | document | navigation |
| 30 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |

Assets: 29 (29 on the site), not listed row by row: 11 stylesheet, 16 script, 1 font, 1 image.

### product

Load a product page. File: `woocommerce/product.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/product/beanie/` | document | navigation |
| 39 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |

Assets: 39 (38 on the site), not listed row by row: 15 stylesheet, 21 script, 1 font, 2 image.

### add-to-cart-product-page

Load a product page and add two to the cart with its add to cart form. File: `woocommerce/add-to-cart-product-page.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/product/beanie/` | document | navigation |
| 39 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |
| 42 | POST | `/product/beanie/` | document | navigation |
| 132 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |

Assets: 130 (128 on the site), not listed row by row: 30 stylesheet, 42 script, 2 font, 4 image, 52 other.

### add-to-cart-shop

Load the shop page and add a simple product with its Add to cart button. File: `woocommerce/add-to-cart-shop.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/shop/` | document | navigation |
| 30 | GET | `/wp-json/wc/store/v1/cart` | xhr | `stores/woocommerce/cart.js` |
| 33 | POST | `/wp-json/wc/store/v1/batch` | xhr | `stores/woocommerce/cart.js` |

Assets: 30 (30 on the site), not listed row by row: 11 stylesheet, 17 script, 1 font, 1 image.

### cart-page

Load the cart page with a product in the cart. File: `woocommerce/cart-page.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/cart/` | document | navigation |

Assets: 124 (124 on the site), not listed row by row: 9 stylesheet, 62 script, 2 image, 50 other, 1 font.

### update-cart-item

On the loaded cart page, raise a line's quantity by one with its + control. File: `woocommerce/update-cart-item.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | POST | `/wp-json/wc/store/v1/cart/update-item?_locale=site` | xhr | `js/dist/api-fetch.min.js` |

Assets: 0 (0 on the site), not listed row by row: none.

### remove-cart-item

On the loaded cart page, remove a line with its remove control. File: `woocommerce/remove-cart-item.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | POST | `/wp-json/wc/store/v1/batch?_locale=site` | xhr | `js/dist/api-fetch.min.js` |

Assets: 0 (0 on the site), not listed row by row: none.

### checkout-page

Load the checkout page with a product in the cart. File: `woocommerce/checkout-page.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/checkout/` | document | navigation |

Assets: 121 (121 on the site), not listed row by row: 9 stylesheet, 59 script, 51 other, 1 font, 1 image.

### place-order

On the loaded checkout page, fill in the address, pay with Cash on delivery and place the order. File: `woocommerce/place-order.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | POST | `/wp-json/wc/store/v1/batch?_locale=site` | xhr | `js/dist/api-fetch.min.js` |
| 2 | POST | `/wp-json/wc/store/v1/batch?_locale=site` | xhr | `js/dist/api-fetch.min.js` |
| 3 | POST | `/wp-json/wc/store/v1/checkout?_locale=site` | xhr | `js/dist/api-fetch.min.js` |
| 4 | GET | `/checkout/order-received/40/?key=wc_order_PWpfiykP0kkOD` | document | navigation |

Assets: 28 (28 on the site), not listed row by row: 12 stylesheet, 15 script, 1 font.

### my-account-log-in

Load My Account and log in with its log in form. File: `woocommerce/my-account-log-in.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/my-account/` | document | navigation |
| 22 | POST | `/my-account/` (302) | document | navigation |
| 23 | GET | `/my-account/` | document | navigation |

Assets: 40 (40 on the site), not listed row by row: 12 stylesheet, 26 script, 2 font.

### my-account-orders

Load My Account's orders list, logged in. File: `woocommerce/my-account-orders.har`.

| # | Method | Path | Type | Sent by |
|---|---|---|---|---|
| 1 | GET | `/my-account/orders/` | document | navigation |

Assets: 20 (20 on the site), not listed row by row: 6 stylesheet, 13 script, 1 font.
