#!/usr/bin/env bash
# Run the WooCommerce example against the WooCommerce demo shop and fail on any
# request or transaction error, then check that the run placed orders. The
# `woocommerce` CI job runs this script unchanged.
#
#   scripts/woocommerce.sh                             # starts the shop, runs, leaves it up
#   WOOCOMMERCE_SITE_RUNNING=1 scripts/woocommerce.sh  # reuse a shop already started
#
# Extra arguments are passed to the example, after the defaults.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"
SITE=examples/wordpress-sites/woocommerce/setup.sh

PORT=${WOOCOMMERCE_PORT:-18082}
RUN_TIME=${WOOCOMMERCE_RUN_TIME:-60s}
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}

command -v jq >/dev/null || {
    echo "woocommerce: jq is required" >&2
    exit 1
}

if [ "${WOOCOMMERCE_SITE_RUNNING:-}" != 1 ]; then
    PORT=$PORT "$SITE" start
fi

orders() {
    PORT=$PORT "$SITE" wp wc shop_order list --user="$ADMIN_USERNAME" --status=any --format=count 2>/dev/null
}
orders_before=$(orders)

REPORT_DIR=$(mktemp -d)
trap 'rm -rf "$REPORT_DIR"' EXIT
REPORT="$REPORT_DIR/woocommerce.json"

# The scenario weights (6, 3 and 1) start one returning customer in every ten
# users, so 30 users run three at once, each logged into its own account.
cargo run --example woocommerce -- \
    --host "http://localhost:${PORT}" \
    --users 30 --increase-rate 10 --run-time "$RUN_TIME" \
    --no-telnet --no-websocket \
    --no-reset-metrics \
    --report-file "$REPORT" \
    "$@"

requests=$(jq '[.raw_metrics.requests[] | .success_count + .fail_count] | add // 0' "$REPORT")
request_failures=$(jq '[.raw_metrics.requests[] | .fail_count] | add // 0' "$REPORT")
transactions=$(jq '[.raw_metrics.transactions[][] | .success_count + .fail_count] | add // 0' "$REPORT")
transaction_failures=$(jq '[.raw_metrics.transactions[][] | .fail_count] | add // 0' "$REPORT")
errors=$(jq '.raw_metrics.errors | length' "$REPORT")
orders_after=$(orders)
placed=$((orders_after - orders_before))

echo "woocommerce: requests ${requests}, request failures ${request_failures}, transactions ${transactions}, transaction failures ${transaction_failures}, distinct errors ${errors}, orders placed ${placed} (${orders_after} in the shop)"

if [ "$requests" -eq 0 ] || [ "$transactions" -eq 0 ]; then
    echo "woocommerce: FAILED, the run made no requests or ran no transactions" >&2
    exit 1
fi
if [ "$request_failures" -ne 0 ] || [ "$transaction_failures" -ne 0 ] || [ "$errors" -ne 0 ]; then
    echo "woocommerce: FAILED, errors in the run:" >&2
    jq -r '.raw_metrics.errors[] | "  \(.occurrences) x \(.method) \(.name): \(.error)"' "$REPORT" >&2
    jq -r '.raw_metrics.transactions[][] | select(.fail_count > 0)
        | "  transaction \(.scenario_name) \(.transaction_index): \(.fail_count) failed"' "$REPORT" >&2
    exit 1
fi
if [ "$placed" -le 0 ]; then
    echo "woocommerce: FAILED, the run placed no orders" >&2
    exit 1
fi
echo "woocommerce: OK"
