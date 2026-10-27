#!/usr/bin/env bash
# Start or stop a Drupal 11 Umami demo site in Docker.
#
# The site serves three things: the Umami example run (scripts/umami.sh, also the
# `umami` CI job), the fixtures the Drupal tests replay (scripts/capture-fixtures.sh),
# and local runs of the example by hand.
#
#   scripts/umami-site.sh start   # site on http://localhost:${UMAMI_PORT:-18080}/
#   scripts/umami-site.sh stop    # remove the containers and network
set -euo pipefail

# Drupal 11.3: from 11.4 core's node search always denies access (moved to the
# contrib search_node module), and the example searches.
DRUPAL_IMAGE=drupal:11.3.20-apache-bookworm
MARIADB_IMAGE=mariadb:11.4.13
DRUSH_VERSION=13.7.7

PORT=${UMAMI_PORT:-18080}
ADMIN_USERNAME=${ADMIN_USERNAME:-admin}
ADMIN_PASSWORD=${ADMIN_PASSWORD:-P@ssw0rd1234}

NETWORK=goose-eggs-umami
DB=goose-eggs-umami-db
WEB=goose-eggs-umami-web

stop() {
    docker rm -f "$WEB" "$DB" >/dev/null 2>&1 || true
    docker network rm "$NETWORK" >/dev/null 2>&1 || true
}

wait_for() {
    local what=$1
    shift
    for _ in $(seq 1 120); do
        if "$@" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    echo "umami-site: timed out waiting for $what" >&2
    exit 1
}

start() {
    stop
    docker network create "$NETWORK" >/dev/null
    docker run -d --name "$DB" --network "$NETWORK" \
        -e MARIADB_DATABASE=drupal -e MARIADB_USER=drupal -e MARIADB_PASSWORD=drupal \
        -e MARIADB_ROOT_PASSWORD=root \
        "$MARIADB_IMAGE" >/dev/null
    docker run -d --name "$WEB" --network "$NETWORK" -p "127.0.0.1:${PORT}:80" \
        -e COMPOSER_ALLOW_SUPERUSER=1 \
        "$DRUPAL_IMAGE" >/dev/null

    wait_for "MariaDB" docker exec "$DB" mariadb-admin ping -udrupal -pdrupal --silent
    docker exec -w /opt/drupal "$WEB" composer require --no-interaction --quiet "drush/drush:${DRUSH_VERSION}"
    # The image's CLI memory limit (128M) is too small to install Umami.
    drush() { docker exec -w /opt/drupal "$WEB" php -d memory_limit=-1 vendor/drush/drush/drush.php "$@"; }
    drush site:install demo_umami \
        --db-url=mysql://drupal:drupal@${DB}/drupal \
        --account-name="$ADMIN_USERNAME" --account-pass="$ADMIN_PASSWORD" \
        --site-name="Umami Food Magazine" -y
    # Umami installs English and Spanish; fail loudly if Spanish is ever missing.
    [ "$(drush php:eval "echo \\Drupal::languageManager()->getLanguage('es') ? 'es' : 'none';")" = es ] || {
        echo "umami-site: Spanish is not enabled" >&2
        exit 1
    }
    # Umami has no contact form; the example posts the site-wide feedback form,
    # so install it as Drupal's standard profile does, with its Spanish label.
    drush pm:install contact -y
    drush php:eval '
        \Drupal\contact\Entity\ContactForm::create([
            "id" => "feedback",
            "label" => "Website feedback",
            "recipients" => ["admin@example.com"],
            "selected" => TRUE,
            "reply" => "",
        ])->save();
        \Drupal::languageManager()
            ->getLanguageConfigOverride("es", "contact.form.feedback")
            ->set("label", "Comentarios sobre el sitio web")
            ->save();
        user_role_grant_permissions("anonymous", ["access site-wide contact form"]);
        user_role_grant_permissions("authenticated", ["access site-wide contact form"]);
    '
    # The example searches for words from node titles; index the content.
    drush cron
    # Build every image derivative now: Drupal answers 503 while another request
    # is generating the same derivative, which concurrent users would hit.
    drush php:eval '
        $files = \Drupal::entityTypeManager()->getStorage("file")->loadMultiple();
        foreach (\Drupal\image\Entity\ImageStyle::loadMultiple() as $style) {
            foreach ($files as $file) {
                $uri = $file->getFileUri();
                if (preg_match("/\\.(jpe?g|png|gif|webp)$/i", $uri)) {
                    $style->createDerivative($uri, $style->buildUri($uri));
                }
            }
        }
    '
    # drush ran as root; Apache must be able to write generated files.
    docker exec "$WEB" chown -R www-data:www-data /opt/drupal/web/sites/default/files
    wait_for "Drupal on port $PORT" curl -sf "http://localhost:${PORT}/user/login"
    echo "umami-site: up on http://localhost:${PORT}/ (admin: ${ADMIN_USERNAME})"
}

case "${1:-}" in
    start) start ;;
    stop) stop ;;
    *)
        echo "usage: $0 start|stop" >&2
        exit 2
        ;;
esac
