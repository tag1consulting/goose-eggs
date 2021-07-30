# Changelog

## 0.1.9-dev
 - introduce drupal-specific `get_form_values` to efficiently load multiple form values
 - allow validation of whether or not request redirected; rework how `Validate` object is built, allowing it to be changed
 - introduce mod `text` offering two helper functions, `random_word` and `random_words`
 - introduce drupal-specific `get_bigpipe_form` to extract a form that has been encoded to replace a BigPipe placeholder

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
