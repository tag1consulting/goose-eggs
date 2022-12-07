# Changelog

## 0.4.2-dev
 - add support for building with [rustls](https://docs.rs/rustls) via the `rustls-tls` feature

## 0.4.1 June 16, 2022
 - introduce `validate_page` to validate a page without loading static assets, an alternative to `validate_and_load_static_assets`

## 0.4.0 May 1, 2022
 - update goose to [0.16](https://github.com/tag1consulting/goose/releases/tag/0.16.0)

## 0.3.1 November 2, 2021
 - update goose to [0.15](https://github.com/tag1consulting/goose/releases/tag/0.15.0)

## 0.3.0 October 26, 2021
 - find forms identified with either `id=` or `data-drupal-selector=`
 - **API change**: introduce `SearchParamsBuilder` to build `SearchParams` objects
    o Replaces `SearchParams::keys`, `SearchParams::update_keys`, `SearchParams::update_url`, `SearchParams::update_submit`, `SearchParams::update_title`
    o Builder pattern is: `SearchParams::builder().keys("foo").url("custom/url").submit("Search").build();`
 - **API change**: introduce `LoginBuilder` to build `Login` objects
    o Replaces `Login::username`, `Login::password`, `Login::username_password`, `Login::url`, `Login::title`, `Login::url_title`, `Login::update_username`, `Login::update_password`, `Login::update_username_password`, `Login::update_url`, `Login::update_title`, `Login::update_url_title`
    o Builder pattern is: `Login::builder().username("username").password("password").url("custom/login/url").build();`
 - **API change**: introduce `ValidateBuilder` to build `Validate` objects
    o Replaces `Validate::new`, `Validate::status`, `Validate::title`, `Validate::text`, `Validate::texts`, `Validate::title_text`, `Validate::title_texts`, `Validate::header`, `Validate::headers`, `Validate::redirect`, `Validate::update_status`, `Validate::update_title`, `Valudate::update_text`, `Validate::update_texts`, `Validate::update_header`, `Validate::update_header`
    o Builder pattern is: `Validate::builder().status(200).text("foo").redirect(true).build();`
 - **API change**: remove `Header` struct, instead using a simple (&str, &str) tuple
    o Removes `Header` and all associated functions
    o Builder pattern to validate headers is `Validate::builder().header("cache").header_value("x-generator", "Drupal 7").build();`

## 0.2.0 October 5, 2021
 - **API change**: update goose to [0.14](https://github.com/tag1consulting/goose/releases/tag/0.14.0)

## 0.1.12 August 22, 2021
 - clippy cleanups: don't borrow references that are immediately dereferenced by the compiler: https://rust-lang.github.io/rust-clippy/master/index.html#needless_borrow
 - update `load_static_elements()` to use case-insensitive regex to find local static elements (images, js, and css) both with relative and absolute paths
 - match user-login-form even when it has additional classes

## 0.1.11 August 4, 2021
 - remove extra and incorrect cut and paste example for `SearchParams::keys`
 - match headers `<head>` that include other attributes, such as `<head profile="..">`

## 0.1.10 August 2, 2021
 - escape form element name so regex compiles if name includes characters such as `[]`
 - introduce drupal-specific `get_encoded_form_values` to efficiently load multiple encoded form values
 - introduce drupal-specific `search` function, configured with `SearchParams` object controlling the search keys, and optionally setting a custom url and title for the search form page, and a custom submit button for the search form
 - add `examples/umami`, converting the example from Goose to use Goose Eggs for load testing Drupal 9 demo install profile

## 0.1.9 July 30, 2021
 - introduce drupal-specific `get_form_values` to efficiently load multiple form values
 - allow validation of whether or not request redirected; rework how `Validate` object is built, allowing it to be changed
 - introduce mod `text` offering two helper functions, `random_word` and `random_words`
 - introduce drupal-specific `get_bigpipe_form` to extract a form that has been encoded to replace a BigPipe placeholder
 - introduce drupal-specific `get_encoded_form_value` to extract a value from an encoded form returned by an ajax callback or a BigPipe placeholder
 - introduce drupal-specific `get_updated_build_id` to update the `build_id` which can happen after certain form actions, such as uploading a file

## 0.1.8 July 26, 2021
 - validate `get_form` and `get_form_value` succeed or throw warn! level log
 - lazy match in `get_form` regex to avoid matching multiple forms

## 0.1.7 July 22, 2021
 - introduce `get_html_header()` helper, and invoke from `valid_title()`
 - introduce `get_title()` helper, and invoke from `valid_title()`
 - update `valid_title()` to verify that the title contains the specified string (whereas before it tested that it started with the specified string)
 - change `USER` to `GOOSE_USER` and `PASS` to `GOOSE_PASS` to avoid conflicts with shell defaults
 - allow override of expected title after user login; rework how `Login` object is built, allowing it to be changed

## 0.1.6 July 20, 2021
 - return loaded html as `String` from `validate_and_load_static_assets()`
 - validate response in the order information comes available (status code, headers, title and texts)
 - introduce drupal-specific `get_form()` and `get_form_value()` in new mod drupal
 - introduce drupal-specific `login()` function and `Login` object to override the default username, password, and url

## 0.1.5 July 19, 2021
 - documentation fix, `load_static_elements()` is `async` and requires `.await`
 - update goose dependency to `0.13`

## 0.1.4 July 18, 2021
 - add `headers` parameter to `Validate` and `header_is_set()` and `valid_header_value()` helper functions; optionally validate headers from `validate_and_load_static_assets()`
 - make `Validate` and `Header` fields private, and provide numerous helpers for conveniently setting them

## 0.1.1, 0.1.2, 0.1.3 July 16, 2021
 - introduce `load_static_elements()`, `valid_text()`, `valid_title()`, `validate_and_load_static_assets()`, and `Validate`; document
 - enable CI
 - improve documentation; add CHANGELOG
