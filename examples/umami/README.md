# Overview

This is the [Umami Goose Example](https://github.com/tag1consulting/goose/tree/main/examples/umami), converted to leverage the Goose Eggs library.

It load tests Drupal's Umami demo profile (`demo_umami`) with three kinds of users: anonymous English and Spanish users who browse every kind of page, search and post the contact form, and an admin who logs in and edits articles.

## Running it locally

`scripts/umami-site.sh` starts a Drupal 11 Umami site in Docker, the same one the `umami` CI job runs against: Drupal 11.3 and MariaDB 11.4 containers, Umami installed with drush, and the site wide contact form the example posts (Umami has none of its own). From the repository root:

```sh
scripts/umami-site.sh start
cargo run --example umami -- --host http://localhost:18080 --users 5 --increase-rate 5 --run-time 60s
scripts/umami-site.sh stop
```

`scripts/umami.sh` does the same as the CI job: it starts the site, runs the example for 60 seconds, and fails on any request or transaction error. Set `UMAMI_SITE_RUNNING=1` to reuse a site that is already up.

The site listens on port 18080; set `UMAMI_PORT` to use another port, for both scripts and for `--host`.

## Admin credentials

The admin user logs in as `admin` with the password `P@ssw0rd1234` unless the `ADMIN_USERNAME` and `ADMIN_PASSWORD` environment variables say otherwise. `scripts/umami-site.sh` creates the admin from the same variables, so setting them once covers both.
