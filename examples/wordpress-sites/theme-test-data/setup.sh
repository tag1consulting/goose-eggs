#!/usr/bin/env bash
# Start or stop a WordPress site loaded with the theme unit test data, in Docker.
#
# The site serves the `wordpress` integration test (tests/wordpress_site.rs, also
# the `wordpress` CI job), the fixtures the WordPress tests replay
# (examples/wordpress-sites/capture-fixtures.sh), and runs of a load test by hand.
#
#   examples/wordpress-sites/theme-test-data/setup.sh start   # http://localhost:${PORT:-18081}/
#   examples/wordpress-sites/theme-test-data/setup.sh stop    # remove containers and volumes
set -euo pipefail

DIR=$(cd "$(dirname "$0")" && pwd)

# The theme unit test data, pinned. The repository moved from WPTRT to WordPress.
TEST_DATA_COMMIT=b47acf980696897936265182cb684dca648476c7
TEST_DATA_URL="https://raw.githubusercontent.com/WordPress/theme-test-data/${TEST_DATA_COMMIT}/themeunittestdata.wordpress.xml"
IMPORTER_VERSION=0.9.7

export PORT=${PORT:-18081}
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-admin-password}
EDITOR_USERNAME=${EDITOR_USERNAME:-editor}
EDITOR_PASSWORD=${EDITOR_PASSWORD:-editor-password}

compose() { docker compose -f "$DIR/compose.yaml" "$@"; }
wp() { compose run --rm -T cli wp "$@"; }

stop() {
    compose --profile cli down --volumes --remove-orphans >/dev/null 2>&1 || true
}

start() {
    stop
    compose up -d --wait db wordpress
    wp core install --url="http://localhost:${PORT}" --title="Theme Test Data" \
        --admin_user="$ADMIN_USERNAME" --admin_password="$ADMIN_PASSWORD" \
        --admin_email=admin@example.com --skip-email
    wp user create "$EDITOR_USERNAME" editor@example.com --role=editor \
        --user_pass="$EDITOR_PASSWORD"
    wp rewrite structure '/%postname%/' --hard
    wp plugin install wordpress-importer --version="$IMPORTER_VERSION" --activate
    # Attachments are skipped: importing them downloads every image from
    # wordpress.com, which makes the site depend on the network and its speed.
    curl -sfL "$TEST_DATA_URL" | compose run --rm -T cli sh -c \
        'cat >/tmp/themeunittestdata.xml && wp import /tmp/themeunittestdata.xml --authors=create --skip=attachment --quiet'
    # Pretty permalinks need Apache's rewrite rules; the image ships them.
    compose exec -T wordpress test -f /var/www/html/.htaccess
    for _ in $(seq 1 60); do
        curl -sf "http://localhost:${PORT}/wp-login.php" >/dev/null && break
        sleep 1
    done
    curl -sf "http://localhost:${PORT}/wp-sitemap.xml" >/dev/null || {
        echo "theme-test-data: the site is not answering on port ${PORT}" >&2
        exit 1
    }
    echo "theme-test-data: up on http://localhost:${PORT}/ ($(wp post list --post_type=post --format=count) posts, $(wp post list --post_type=page --format=count) pages)"
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
