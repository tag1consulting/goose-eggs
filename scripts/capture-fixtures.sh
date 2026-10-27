#!/usr/bin/env bash
# Capture the Drupal fixtures that tests/drupal.rs replays through httpmock.
#
# Needs the Umami site from scripts/umami-site.sh. Pages are saved as the site
# served them, with the site's absolute URLs rewritten to relative ones so the
# fixtures do not depend on the port they were captured from.
#
#   scripts/umami-site.sh start
#   scripts/capture-fixtures.sh
set -euo pipefail

PORT=${UMAMI_PORT:-18080}
BASE="http://localhost:${PORT}"
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-P@ssw0rd1234}
OUT=tests/fixtures/drupal

mkdir -p "$OUT"
JAR=$(mktemp)
trap 'rm -f "$JAR"' EXIT

fetch() {
    # fetch <path> <fixture file>: GET with the session cookie, fail on non 200.
    curl -sf -b "$JAR" -c "$JAR" "${BASE}/$1" | sed "s#${BASE}/#/#g" >"$OUT/$2"
}

form_value() {
    # form_value <fixture file> <form element name>
    grep -o "name=\"$2\" value=\"[^\"]*\"" "$OUT/$1" | head -1 | sed 's/.*value="//; s/"$//'
}

# The log in page, and the page Drupal redirects to after a good log in.
fetch en/user/login user-login.html
status=$(curl -s -b "$JAR" -c "$JAR" -o /dev/null -w '%{http_code}' \
    --data-urlencode "name=${ADMIN_USERNAME}" \
    --data-urlencode "pass=${ADMIN_PASSWORD}" \
    --data-urlencode "form_build_id=$(form_value user-login.html form_build_id)" \
    --data-urlencode "form_id=user_login_form" \
    --data-urlencode "op=Log in" \
    "${BASE}/en/user/login")
if [ "$status" != 303 ]; then
    echo "capture-fixtures: log in answered $status, not 303" >&2
    exit 1
fi
fetch en/user/1 user-logged-in.html

# A log in with the wrong password: Drupal answers 200 with the form again.
WRONG_JAR=$(mktemp)
curl -sf -c "$WRONG_JAR" "${BASE}/en/user/login" >/dev/null
curl -sf -b "$WRONG_JAR" \
    --data-urlencode "name=${ADMIN_USERNAME}" \
    --data-urlencode "pass=not-the-password" \
    --data-urlencode "form_build_id=$(form_value user-login.html form_build_id)" \
    --data-urlencode "form_id=user_login_form" \
    --data-urlencode "op=Log in" \
    "${BASE}/en/user/login" | sed "s#${BASE}/#/#g" >"$OUT/user-login-failed.html"
rm -f "$WRONG_JAR"

# The search form and a results page.
fetch en/search/node search.html
fetch "en/search/node?keys=mushrooms" search-results.html

# An article edit form, and the AJAX response to adding a tag to it, which
# carries Drupal's update_build_id command.
fetch en/node/15/edit node-edit.html
curl -sf -b "$JAR" \
    -H 'X-Requested-With: XMLHttpRequest' \
    --data-urlencode "form_build_id=$(form_value node-edit.html form_build_id)" \
    --data-urlencode "form_token=$(form_value node-edit.html form_token)" \
    --data-urlencode "form_id=node_article_edit_form" \
    --data-urlencode "_triggering_element_name=field_media_image-media-library-update" \
    --data-urlencode "media-library-modal-selection=1" \
    "${BASE}/en/node/15/edit?ajax_form=1&_wrapper_format=drupal_ajax&element_parents=field_media_image/widget" \
    | sed "s#${BASE}/#/#g" >"$OUT/node-edit-ajax.json"

echo "capture-fixtures: wrote $(ls "$OUT" | wc -l | tr -d ' ') files to $OUT"
