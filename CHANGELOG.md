# Changelog

## 0.1.6-dev
 - return loaded html as `String` from `validate_and_load_static_assets()`
 - validate response in the order information comes available (status code, headers, title and texts)

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
