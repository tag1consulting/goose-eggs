#!/usr/bin/env bash
# Start or stop a WooCommerce shop with WooCommerce's sample products, in Docker.
#
# The shop serves the WooCommerce example (examples/woocommerce, also the
# `woocommerce` CI job), the fixtures the WooCommerce tests replay
# (examples/wordpress-sites/capture-fixtures.sh), and runs of the example by hand.
#
#   examples/wordpress-sites/woocommerce/setup.sh start   # http://localhost:${PORT:-18082}/
#   examples/wordpress-sites/woocommerce/setup.sh stop    # remove containers and volumes
#   examples/wordpress-sites/woocommerce/setup.sh wp ...  # run wp-cli against the shop
set -euo pipefail

DIR=$(cd "$(dirname "$0")" && pwd)

WOOCOMMERCE_VERSION=11.2.1
IMPORTER_VERSION=0.9.7

export PORT=${PORT:-18082}
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-admin-password}
EDITOR_USERNAME=${EDITOR_USERNAME:-editor}
EDITOR_PASSWORD=${EDITOR_PASSWORD:-editor-password}
CUSTOMER_USERNAME=${CUSTOMER_USERNAME:-customer}
CUSTOMER_PASSWORD=${CUSTOMER_PASSWORD:-customer-password}

compose() { docker compose -f "$DIR/compose.yaml" "$@"; }
wp() { compose run --rm -T cli wp "$@"; }

stop() {
    compose --profile cli down --volumes --remove-orphans >/dev/null 2>&1 || true
}

start() {
    stop
    compose up -d --quiet-pull --wait db wordpress
    wp core install --url="http://localhost:${PORT}" --title="WooCommerce Demo" \
        --admin_user="$ADMIN_USERNAME" --admin_password="$ADMIN_PASSWORD" \
        --admin_email=admin@example.com --skip-email
    wp user create "$EDITOR_USERNAME" editor@example.com --role=editor \
        --user_pass="$EDITOR_PASSWORD"
    wp rewrite structure '/%postname%/' --hard
    wp plugin install wordpress-importer --version="$IMPORTER_VERSION" --activate
    wp plugin install woocommerce --version="$WOOCOMMERCE_VERSION" --activate

    # The store: open to everyone (new stores start in "coming soon" mode), in
    # the US in dollars, without stock management so a long run never sells out.
    wp option update woocommerce_coming_soon no
    wp option update woocommerce_store_pages_only no
    wp option update woocommerce_default_country 'US:CA'
    wp option update woocommerce_currency USD
    wp option update woocommerce_manage_stock no
    wp option update woocommerce_enable_guest_checkout yes
    wp option update woocommerce_enable_myaccount_registration yes
    wp option update woocommerce_onboarding_profile '{"skipped":true}' --format=json
    # Shop, cart, checkout and my account pages, as WooCommerce's setup creates
    # them: the cart and checkout are blocks.
    wp wc tool run install_pages --user="$ADMIN_USERNAME"
    # The sample products shipped inside the plugin.
    wp import wp-content/plugins/woocommerce/sample-data/sample_products.xml \
        --authors=skip --skip=attachment --quiet >/dev/null
    # Shoppers check out with cash on delivery and free shipping everywhere.
    wp wc payment_gateway update cod --enabled=true --user="$ADMIN_USERNAME" >/dev/null
    wp wc shipping_zone_method create 0 --method_id=free_shipping --enabled=true \
        --user="$ADMIN_USERNAME" >/dev/null
    # A customer with an address on file, for the returning customer.
    wp wc customer create --email=customer@example.com --username="$CUSTOMER_USERNAME" \
        --password="$CUSTOMER_PASSWORD" --first_name=Casey --last_name=Customer \
        --billing='{"first_name":"Casey","last_name":"Customer","address_1":"1 Main Street","city":"San Francisco","state":"CA","postcode":"94105","country":"US","email":"customer@example.com","phone":"5555550100"}' \
        --user="$ADMIN_USERNAME" >/dev/null
    # Shortcode cart and checkout pages, for the classic flows. The block pages
    # above stay WooCommerce's cart and checkout.
    wp post create --post_type=page --post_status=publish --post_title='Classic Cart' \
        --post_name=classic-cart --post_content='[woocommerce_cart]' >/dev/null
    wp post create --post_type=page --post_status=publish --post_title='Classic Checkout' \
        --post_name=classic-checkout --post_content='[woocommerce_checkout]' >/dev/null
    wp rewrite flush

    for _ in $(seq 1 60); do
        curl -sf "http://localhost:${PORT}/shop/" >/dev/null && break
        sleep 1
    done
    curl -sf "http://localhost:${PORT}/wp-json/wc/store/v1/products" >/dev/null || {
        echo "woocommerce: the shop is not answering on port ${PORT}" >&2
        exit 1
    }
    echo "woocommerce: up on http://localhost:${PORT}/ ($(wp post list --post_type=product --format=count) products)"
}

case "${1:-}" in
    start) start ;;
    stop) stop ;;
    wp)
        shift
        wp "$@"
        ;;
    *)
        echo "usage: $0 start|stop|wp <args>" >&2
        exit 2
        ;;
esac
