#!/usr/bin/env bash
# Run the `wordpress` integration test (tests/sites/wordpress.rs) against the
# theme test data site. The `wordpress` CI job runs this script unchanged.
#
#   scripts/wordpress.sh                           # starts the site, tests, leaves it up
#   WORDPRESS_SITE_RUNNING=1 scripts/wordpress.sh  # reuse a site already started
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

PORT=${WORDPRESS_PORT:-18081}

if [ "${WORDPRESS_SITE_RUNNING:-}" != 1 ]; then
    PORT=$PORT examples/wordpress-sites/theme-test-data/setup.sh start
fi

WORDPRESS_SITE="http://localhost:${PORT}" \
    cargo test --all-features --test wordpress_site -- --nocapture
