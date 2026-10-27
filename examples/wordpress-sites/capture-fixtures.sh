#!/usr/bin/env bash
# Capture the fixtures that the WordPress and WooCommerce tests replay through
# httpmock, from the two demo sites.
#
# Pages are saved as the sites served them, with the site's absolute URLs (plain
# and JSON escaped) rewritten to relative ones so the fixtures do not depend on
# the port they were captured from. Where a test needs a response header (a
# nonce, a cart token, a page count), the headers are saved next to the body as
# <fixture>.headers.
#
#   examples/wordpress-sites/theme-test-data/setup.sh start
#   examples/wordpress-sites/woocommerce/setup.sh start
#   examples/wordpress-sites/capture-fixtures.sh
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
SITES=examples/wordpress-sites

WP_BASE="http://localhost:${WORDPRESS_PORT:-18081}"
WC_BASE="http://localhost:${WOOCOMMERCE_PORT:-18082}"
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-admin-password}
CUSTOMER_USERNAME=${CUSTOMER_USERNAME:-customer}
CUSTOMER_PASSWORD=${CUSTOMER_PASSWORD:-customer-password}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# relative <base>: rewrite the site's absolute URLs on stdin to relative ones.
relative() {
    BASE=$1 perl -pe 'BEGIN { $b = $ENV{BASE}; ($j = $b) =~ s#/#\\/#g } s#\Q$b\E/#/#g; s#\Q$j\E\\/#\\/#g'
}

# capture <base> <jar> <fixture> <curl args...>: save the body, relative, and
# keep the response headers in $TMP/headers for save_headers.
capture() {
    local base=$1 jar=$2 out=$3
    shift 3
    curl -s -b "$jar" -c "$jar" -D "$TMP/headers" "$@" | relative "$base" >"$out"
}

# save_headers <fixture> <header>...: keep the named response headers.
save_headers() {
    local out=$1
    shift
    local pattern
    pattern=$(printf '|%s' "$@")
    tr -d '\r' <"$TMP/headers" | grep -iE "^(${pattern:1}):" | relative "$WP_BASE" >"$out.headers"
}

# value <file> <field name>: a hidden field's value, attributes in any order.
value() {
    perl -0ne 'while (/<input\b([^>]*)>/sg) { my $a = $1; if ($a =~ /\bname=["\x27]\Q'"$2"'\E["\x27]/ && $a =~ /\bvalue=["\x27]([^"\x27]*)["\x27]/) { print $1; exit } }' "$1"
}

header() {
    tr -d '\r' <"$TMP/headers" | awk -F': ' -v h="$(tr '[:upper:]' '[:lower:]' <<<"$1")" 'tolower($1) == h { print $2 }'
}

wordpress() {
    local out=tests/fixtures/wordpress base=$WP_BASE
    mkdir -p "$out"
    local jar="$TMP/wp-admin" anon="$TMP/wp-anon"

    # Log in: the form, a wrong password, a missing test cookie, the dashboard.
    capture "$base" "$jar" "$out/login.html" "$base/wp-login.php"
    save_headers "$out/login.html" Set-Cookie
    capture "$base" "$TMP/wp-wrong" "$out/login-failed.html" "$base/wp-login.php"
    capture "$base" "$TMP/wp-wrong" "$out/login-failed.html" \
        --data-urlencode "log=${ADMIN_USERNAME}" --data-urlencode "pwd=not-the-password" \
        --data-urlencode "wp-submit=Log In" --data-urlencode "redirect_to=${base}/wp-admin/" \
        --data-urlencode "testcookie=1" "$base/wp-login.php"
    capture "$base" "$TMP/wp-nocookie" "$out/login-no-test-cookie.html" \
        --data-urlencode "log=${ADMIN_USERNAME}" --data-urlencode "pwd=${ADMIN_PASSWORD}" \
        --data-urlencode "wp-submit=Log In" --data-urlencode "redirect_to=${base}/wp-admin/" \
        --data-urlencode "testcookie=1" "$base/wp-login.php"
    curl -s -b "$jar" -c "$jar" -o /dev/null \
        --data-urlencode "log=${ADMIN_USERNAME}" --data-urlencode "pwd=${ADMIN_PASSWORD}" \
        --data-urlencode "wp-submit=Log In" --data-urlencode "redirect_to=${base}/wp-admin/" \
        --data-urlencode "testcookie=1" "$base/wp-login.php"
    capture "$base" "$jar" "$out/dashboard.html" "$base/wp-admin/"

    # Comments: the form anonymous and logged in, then WordPress's answers to a
    # duplicate and to a comment posted too quickly after another.
    capture "$base" "$anon" "$out/comment-form-anonymous.html" "$base/hello-world/"
    capture "$base" "$jar" "$out/comment-form-logged-in.html" "$base/hello-world/"
    # WordPress refuses a second comment from one address within 15 seconds, so
    # wait out any comment posted just before (an earlier capture, a test run).
    sleep 16
    local text="Captured comment $(date +%s)"
    curl -s -b "$anon" -c "$anon" -o /dev/null --data-urlencode "comment=$text" \
        --data author=Goose --data email=goose@example.com --data comment_post_ID=1 \
        --data comment_parent=0 "$base/wp-comments-post.php"
    capture "$base" "$anon" "$out/comment-duplicate.html" --data-urlencode "comment=$text" \
        --data author=Goose --data email=goose@example.com --data comment_post_ID=1 \
        --data comment_parent=0 "$base/wp-comments-post.php"
    capture "$base" "$anon" "$out/comment-flood.html" --data-urlencode "comment=$text, again" \
        --data author=Goose --data email=goose@example.com --data comment_post_ID=1 \
        --data comment_parent=0 "$base/wp-comments-post.php"

    # Search, and log out: a stale nonce asks to confirm, the real one logs out.
    capture "$base" "$anon" "$out/search-results.html" "$base/?s=post"
    capture "$base" "$jar" "$out/logout-confirm.html" \
        "$base/wp-login.php?action=logout&_wpnonce=0000000000"
    capture "$base" "$TMP/wp-gone" "$out/logged-out.html" "$base/wp-login.php?loggedout=true"

    # Content discovery: the sitemap index, a sub sitemap, and a REST list with
    # its paging headers.
    capture "$base" "$anon" "$out/sitemap.xml" "$base/wp-sitemap.xml"
    capture "$base" "$anon" "$out/sitemap-posts-post-1.xml" "$base/wp-sitemap-posts-post-1.xml"
    capture "$base" "$anon" "$out/rest-posts.json" "$base/wp-json/wp/v2/posts?per_page=10&page=1&_fields=link"
    save_headers "$out/rest-posts.json" X-WP-Total X-WP-TotalPages

    # Editing through REST: an update with the dashboard's nonce, and a stale nonce.
    local nonce
    nonce=$(grep -o 'createNonceMiddleware( "[^"]*"' "$out/dashboard.html" | sed 's/.*"\(.*\)"/\1/')
    capture "$base" "$jar" "$out/rest-post-update.json" -H "X-WP-Nonce: $nonce" \
        -H 'Content-Type: application/json' -d '{"title":"Hello world!"}' "$base/wp-json/wp/v2/posts/1"
    capture "$base" "$jar" "$out/rest-invalid-nonce.json" -H "X-WP-Nonce: 0000000000" \
        -H 'Content-Type: application/json' -d '{"title":"Hello world!"}' "$base/wp-json/wp/v2/posts/1"

    echo "capture-fixtures: wrote $(ls "$out" | wc -l | tr -d ' ') files to $out"
}

woocommerce() {
    local out=tests/fixtures/woocommerce base=$WC_BASE
    mkdir -p "$out"
    local jar="$TMP/wc-shopper"
    local product
    product=$(curl -sf "$base/wp-json/wc/store/v1/products?per_page=100" |
        jq -r '[.[] | select(.type == "simple")] | first | .id')
    local permalink
    permalink=$(curl -sf "$base/wp-json/wc/store/v1/products/$product" | jq -r .permalink)

    # Browsing.
    capture "$base" "$jar" "$out/shop.html" "$base/shop/"
    capture "$base" "$jar" "$out/product.html" "$permalink"
    capture "$base" "$jar" "$out/product-search.html" "$base/?s=hoodie&post_type=product"
    capture "$base" "$jar" "$out/store-products.json" "$base/wp-json/wc/store/v1/products?per_page=5&page=1"
    save_headers "$out/store-products.json" X-WP-Total X-WP-TotalPages

    # The Store API cart: its nonce and token, adding an item, a product that
    # does not exist, one out of stock, then checkout with a bad address and a good one.
    capture "$base" "$jar" "$out/store-cart.json" "$base/wp-json/wc/store/v1/cart"
    save_headers "$out/store-cart.json" Nonce Cart-Token
    local nonce
    nonce=$(header Nonce)
    capture "$base" "$jar" "$out/store-add-item-missing-nonce.json" -H 'Content-Type: application/json' \
        -d "{\"id\":$product,\"quantity\":1}" "$base/wp-json/wc/store/v1/cart/add-item"
    capture "$base" "$jar" "$out/store-add-item.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d "{\"id\":$product,\"quantity\":2}" "$base/wp-json/wc/store/v1/cart/add-item"
    save_headers "$out/store-add-item.json" Nonce
    capture "$base" "$jar" "$out/store-add-item-invalid.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d '{"id":999999,"quantity":1}' "$base/wp-json/wc/store/v1/cart/add-item"
    local other
    other=$(curl -sf "$base/wp-json/wc/store/v1/products?per_page=100" |
        jq -r "[.[] | select(.type == \"simple\" and .id != $product)] | first | .id")
    "$SITES/woocommerce/setup.sh" wp eval "\$p = wc_get_product($other); \$p->set_stock_status('outofstock'); \$p->save();"
    capture "$base" "$jar" "$out/store-add-item-out-of-stock.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d "{\"id\":$other,\"quantity\":1}" "$base/wp-json/wc/store/v1/cart/add-item"
    "$SITES/woocommerce/setup.sh" wp eval "\$p = wc_get_product($other); \$p->set_stock_status('instock'); \$p->save();"
    local key
    key=$(jq -r '.items[0].key' "$out/store-add-item.json")
    capture "$base" "$jar" "$out/store-update-item.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d "{\"key\":\"$key\",\"quantity\":3}" "$base/wp-json/wc/store/v1/cart/update-item"
    capture "$base" "$jar" "$out/store-checkout-invalid.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d '{"billing_address":{"first_name":"Goose","email":"not-an-email"},"payment_method":"cod"}' \
        "$base/wp-json/wc/store/v1/checkout"
    local address='{"first_name":"Goose","last_name":"Shopper","address_1":"1 Main Street","city":"San Francisco","state":"CA","postcode":"94105","country":"US","email":"shopper@example.com","phone":"5555550100"}'
    capture "$base" "$jar" "$out/store-checkout.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d "{\"billing_address\":$address,\"shipping_address\":$address,\"payment_method\":\"cod\"}" \
        "$base/wp-json/wc/store/v1/checkout"
    capture "$base" "$jar" "$out/store-remove-item.json" -H "Nonce: $nonce" -H 'Content-Type: application/json' \
        -d '{"key":"0000"}' "$base/wp-json/wc/store/v1/cart/remove-item"

    # The classic flows, on the shortcode cart and checkout pages.
    local classic="$TMP/wc-classic"
    capture "$base" "$classic" "$out/classic-add-to-cart.html" "$base/classic-cart/?add-to-cart=$product&quantity=2"
    capture "$base" "$classic" "$out/classic-add-to-cart-invalid.html" "$base/classic-cart/?add-to-cart=999999"
    capture "$base" "$classic" "$out/classic-cart.html" "$base/classic-cart/"
    local cart_nonce cart_key
    cart_nonce=$(value "$out/classic-cart.html" woocommerce-cart-nonce)
    cart_key=$(grep -o 'name="cart\[[a-f0-9]*\]\[qty\]"' "$out/classic-cart.html" | head -1 | sed 's/name="cart\[//; s/\].*//')
    capture "$base" "$classic" "$out/classic-cart-updated.html" \
        --data "cart%5B${cart_key}%5D%5Bqty%5D=3&update_cart=Update+cart" \
        --data "woocommerce-cart-nonce=$cart_nonce&_wp_http_referer=%2Fclassic-cart%2F" \
        "$base/classic-cart/"
    capture "$base" "$classic" "$out/classic-checkout.html" "$base/classic-checkout/"
    local checkout_nonce
    checkout_nonce=$(value "$out/classic-checkout.html" woocommerce-process-checkout-nonce)
    capture "$base" "$classic" "$out/classic-checkout-failure.json" \
        --data "billing_first_name=Goose&billing_email=not-an-email&payment_method=cod" \
        --data "woocommerce-process-checkout-nonce=$checkout_nonce" "$base/?wc-ajax=checkout"
    capture "$base" "$classic" "$out/classic-checkout-success.json" \
        --data "billing_first_name=Goose&billing_last_name=Shopper&billing_country=US" \
        --data "billing_address_1=1+Main+Street&billing_city=San+Francisco&billing_state=CA" \
        --data "billing_postcode=94105&billing_phone=5555550100&billing_email=shopper%40example.com" \
        --data "payment_method=cod&shipping_method%5B0%5D=free_shipping%3A1" \
        --data "woocommerce-process-checkout-nonce=$checkout_nonce&_wp_http_referer=%2Fclassic-checkout%2F" \
        "$base/?wc-ajax=checkout"
    local received
    received=$(jq -r .redirect "$out/classic-checkout-success.json")
    capture "$base" "$classic" "$out/order-received.html" "$base$received"

    # My Account: the log in form, a wrong password, then the dashboard and orders.
    local customer="$TMP/wc-customer"
    capture "$base" "$customer" "$out/my-account-login.html" "$base/my-account/"
    local login_nonce
    login_nonce=$(value "$out/my-account-login.html" woocommerce-login-nonce)
    capture "$base" "$customer" "$out/my-account-login-failed.html" \
        --data-urlencode "username=${CUSTOMER_USERNAME}" --data-urlencode "password=not-the-password" \
        --data "woocommerce-login-nonce=$login_nonce&_wp_http_referer=%2Fmy-account%2F&login=Log+in" \
        "$base/my-account/"
    curl -s -b "$customer" -c "$customer" -o /dev/null \
        --data-urlencode "username=${CUSTOMER_USERNAME}" --data-urlencode "password=${CUSTOMER_PASSWORD}" \
        --data "woocommerce-login-nonce=$login_nonce&_wp_http_referer=%2Fmy-account%2F&login=Log+in" \
        "$base/my-account/"
    capture "$base" "$customer" "$out/my-account.html" "$base/my-account/"
    capture "$base" "$customer" "$out/my-account-orders.html" "$base/my-account/orders/"

    echo "capture-fixtures: wrote $(ls "$out" | wc -l | tr -d ' ') files to $out"
}

wordpress
WP_BASE=$WC_BASE woocommerce
