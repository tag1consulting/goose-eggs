#!/usr/bin/env bash
# Run the Umami example against a real Drupal 11 Umami site and fail on any
# request or transaction error, including the admin log in during the ramp up
# (so the metrics are never reset). The `umami` CI job runs this script unchanged.
#
#   scripts/umami.sh                       # starts the site, runs, leaves it up
#   UMAMI_SITE_RUNNING=1 scripts/umami.sh  # reuse a site already started
#
# Extra arguments are passed to the example, after the defaults.
set -euo pipefail

PORT=${UMAMI_PORT:-18080}
RUN_TIME=${UMAMI_RUN_TIME:-60s}
export ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
export ADMIN_PASSWORD=${ADMIN_PASSWORD:-P@ssw0rd1234}

command -v jq >/dev/null || {
    echo "umami: jq is required" >&2
    exit 1
}

if [ "${UMAMI_SITE_RUNNING:-}" != 1 ]; then
    scripts/umami-site.sh start
fi

REPORT_DIR=$(mktemp -d)
trap 'rm -rf "$REPORT_DIR"' EXIT
REPORT="$REPORT_DIR/umami.json"

cargo run --example umami -- \
    --host "http://localhost:${PORT}" \
    --users 5 --increase-rate 5 --run-time "$RUN_TIME" \
    --no-telnet --no-websocket \
    --no-reset-metrics \
    --report-file "$REPORT" \
    "$@"

requests=$(jq '[.raw_metrics.requests[] | .success_count + .fail_count] | add // 0' "$REPORT")
request_failures=$(jq '[.raw_metrics.requests[] | .fail_count] | add // 0' "$REPORT")
transactions=$(jq '[.raw_metrics.transactions[][] | .success_count + .fail_count] | add // 0' "$REPORT")
transaction_failures=$(jq '[.raw_metrics.transactions[][] | .fail_count] | add // 0' "$REPORT")
errors=$(jq '.raw_metrics.errors | length' "$REPORT")

echo "umami: requests ${requests}, request failures ${request_failures}, transactions ${transactions}, transaction failures ${transaction_failures}, distinct errors ${errors}"

if [ "$requests" -eq 0 ] || [ "$transactions" -eq 0 ]; then
    echo "umami: FAILED, the run made no requests or ran no transactions" >&2
    exit 1
fi
if [ "$request_failures" -ne 0 ] || [ "$transaction_failures" -ne 0 ] || [ "$errors" -ne 0 ]; then
    echo "umami: FAILED, errors in the run:" >&2
    jq -r '.raw_metrics.errors[] | "  \(.occurrences) x \(.method) \(.name): \(.error)"' "$REPORT" >&2
    jq -r '.raw_metrics.transactions[][] | select(.fail_count > 0)
        | "  transaction \(.scenario_name) \(.transaction_index): \(.fail_count) failed"' "$REPORT" >&2
    exit 1
fi
echo "umami: OK"
