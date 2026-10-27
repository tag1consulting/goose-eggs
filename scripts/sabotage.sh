#!/usr/bin/env bash
# Prove every test by breaking the code it guards.
#
# Each row of tests/sabotage/LEDGER.tsv names a patch that makes one small break
# to the library under src/, the one test that must catch it, and text the
# failure must print. For each row in order this applies the patch, runs only
# that test, and counts the row CAUGHT only when the build succeeded, the test
# failed and its output holds the expected text; anything else (a pass, a build
# error, a timeout, a failure with other text) is NOT CAUGHT. Then it checks that
# every test cargo lists is named by a row, and that every public item is
# covered by a row.
#
#   scripts/sabotage.sh            # every row, then the completeness checks
#
# Exits nonzero unless every row is caught and nothing is untested or uncovered.
set -euo pipefail

LEDGER=${SABOTAGE_LEDGER:-tests/sabotage/LEDGER.tsv}
TIMEOUT=${SABOTAGE_TIMEOUT:-600}

# The public items every ledger must cover, as named in the covers column.
ITEMS=(
    Validate ValidateBuilder status not_status title not_title text not_text texts
    not_texts header not_header header_value not_header_value redirect build none
    get_html_header get_title valid_title valid_text header_is_set valid_header_value
    get_src_elements get_css_elements load_static_elements validate_page
    validate_and_load_static_assets
    get_form get_form_value get_bigpipe_form get_encoded_form_value get_updated_build_id
    get_form_values get_encoded_form_values Login LoginBuilder log_in SearchParams
    SearchParamsBuilder search
    random_word random_words
    wordpress::get_form wordpress::get_form_value wordpress::get_form_values
    wordpress::get_nonce wordpress::get_rest_nonce wordpress::get_log_out_url
    wordpress::Login wordpress::LoginBuilder wordpress::log_in wordpress::log_out
    wordpress::SearchParams wordpress::SearchParamsBuilder wordpress::search
    wordpress::Comment wordpress::CommentBuilder wordpress::post_comment
    wordpress::Post wordpress::PostBuilder wordpress::create_post wordpress::update_post
    wordpress::SiteContent wordpress::Discover wordpress::DiscoverBuilder wordpress::discover
    woocommerce::Shop woocommerce::ShopBuilder woocommerce::shop_page
    woocommerce::product_page woocommerce::product_search woocommerce::Product
    woocommerce::discover_products woocommerce::random_product woocommerce::CartItem
    woocommerce::StoreApiCart woocommerce::Checkout woocommerce::CheckoutBuilder
    woocommerce::classic_add_to_cart woocommerce::classic_update_cart
    woocommerce::classic_checkout woocommerce::my_account_log_in woocommerce::my_account_orders
)

cd "$(git rev-parse --show-toplevel)"

clean_tree() {
    git diff --quiet && git diff --cached --quiet
}

APPLIED=""
restore() {
    if [ -n "$APPLIED" ]; then
        git apply -R "$APPLIED" || true
    fi
}
trap restore EXIT

# Run a command, killing it after $TIMEOUT seconds; prints "timeout" on a kill.
run_with_timeout() {
    "$@" &
    local pid=$!
    (sleep "$TIMEOUT" && kill "$pid" 2>/dev/null && echo "sabotage: timeout after ${TIMEOUT}s") &
    local watchdog=$!
    local status=0
    wait "$pid" || status=$?
    kill "$watchdog" 2>/dev/null || true
    wait "$watchdog" 2>/dev/null || true
    return "$status"
}

list_tests() {
    local target=$1
    case "$target" in
        lib) cargo test -q --all-features --lib -- --list 2>/dev/null ;;
        doc) cargo test -q --all-features --doc -- --list 2>/dev/null ;;
        *) cargo test -q --all-features --test "$target" -- --list 2>/dev/null ;;
    esac | sed -n 's/: test$//p'
}

# rustdoc splits a doc test filter on whitespace and runs any test matching any
# piece, so a doc test's full name selects them all. Filter on the space free
# piece of its name that the fewest doc tests contain (usually only it); the
# verdict below reads the named test's own result either way.
doc_filter() {
    local name=$1 best="" best_count=999999 piece count
    for piece in $name; do
        count=$(grep -c -F -- "$piece" <<<"$DOC_TESTS" || true)
        if [ "$count" -ge 1 ] && [ "$count" -lt "$best_count" ]; then
            best=$piece
            best_count=$count
        fi
    done
    printf '%s' "$best"
}

run_test() {
    local target=$1 test=$2
    case "$target" in
        lib) cargo test --all-features --lib -- "$test" --exact ;;
        doc) cargo test --all-features --doc -- "$(doc_filter "$test")" ;;
        *) cargo test --all-features --test "$target" -- "$test" --exact ;;
    esac
}

# The failure output libtest prints for the test named $1.
failure_of() {
    awk -v head="---- $1 stdout ----" '
        $0 == head { inside = 1; next }
        inside && (/^---- .* stdout ----$/ || /^failures:$/) { exit }
        inside { print }' "$output"
}

if ! clean_tree; then
    echo "sabotage: the working tree has changes; commit or stash them first" >&2
    exit 1
fi

DOC_TESTS=$(list_tests doc)
rows=0
caught=0
not_caught=0
start=$(date +%s)
output=$(mktemp)

while IFS=$'\t' read -r patch target test expected covers; do
    case "$patch" in '' | '#'*) continue ;; esac
    rows=$((rows + 1))
    clean_tree || { echo "sabotage: tree not clean before $patch" >&2; exit 1; }
    if ! git apply "tests/sabotage/$patch"; then
        echo "sabotage: $patch does not apply" >&2
        exit 1
    fi
    APPLIED="tests/sabotage/$patch"

    run_with_timeout run_test "$target" "$test" </dev/null >"$output" 2>&1 || true

    if grep -q 'sabotage: timeout' "$output"; then
        verdict="NOT CAUGHT (timeout)"
    elif grep -q -e 'error: could not compile' -e "Couldn't compile the test" "$output"; then
        verdict="NOT CAUGHT (build failed)"
    elif ! grep -q -x -F -e "test $test ... ok" -e "test $test ... FAILED" "$output"; then
        verdict="NOT CAUGHT (test did not run)"
    elif ! grep -q -x -F "test $test ... FAILED" "$output" || ! grep -q 'test result: FAILED' "$output"; then
        verdict="NOT CAUGHT (test passed)"
    elif ! failure_of "$test" | grep -q -F -- "$expected"; then
        verdict="NOT CAUGHT (failed without: $expected)"
    else
        verdict="CAUGHT"
    fi
    if [ "$verdict" = CAUGHT ]; then
        caught=$((caught + 1))
    else
        not_caught=$((not_caught + 1))
    fi
    echo "$verdict  $target $test  ($patch)"

    git apply -R "tests/sabotage/$patch"
    APPLIED=""
    clean_tree || { echo "sabotage: tree not clean after $patch" >&2; exit 1; }
done <"$LEDGER"
rm -f "$output"

# Completeness: every test cargo lists is named by a row with its target.
untested=0
targets=(lib doc)
for file in tests/*.rs; do
    targets+=("$(basename "$file" .rs)")
done
for target in "${targets[@]}"; do
    while IFS= read -r test; do
        if ! awk -F'\t' -v t="$target" -v n="$test" '$2 == t && $3 == n { found = 1 } END { exit !found }' "$LEDGER"; then
            echo "UNTESTED  $target $test  (no ledger row names it)"
            untested=$((untested + 1))
        fi
    done < <(list_tests "$target")
done

# Completeness: every public item is covered by a row.
uncovered=0
for item in "${ITEMS[@]}"; do
    if ! awk -F'\t' -v i="$item" '$1 !~ /^#/ { n = split($5, c, ","); for (k = 1; k <= n; k++) if (c[k] == i) found = 1 } END { exit !found }' "$LEDGER"; then
        echo "UNCOVERED  $item  (no ledger row covers it)"
        uncovered=$((uncovered + 1))
    fi
done

echo "sabotage: $(($(date +%s) - start))s"
echo "rows $rows, caught $caught, not caught $not_caught, untested tests $untested, uncovered items $uncovered"
[ "$not_caught" -eq 0 ] && [ "$untested" -eq 0 ] && [ "$uncovered" -eq 0 ]
