//! Functionality that's specific to Drupal.

use goose::prelude::*;
use log::warn;
use regex::Regex;
use std::collections::HashMap;
use std::env;

/// Use a regular expression to get the specific form identified by data-drupal-selector.
///
/// See [`get_bigpipe_form`] for a way to extract a form that's served as a BigPipe placeholder.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::get_form;
///
/// // For this example we grab just a subset of a real Drupal form, enough to demonstrate. Normally
/// // you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr" class="light-mode">
///   <form class="user-login-form" data-drupal-selector="user-login-form" action="/user/login" method="post" id="user-login-form" accept-charset="UTF-8">
///     <div class="js-form-item form-item">
///       <label for="edit-name" class="js-form-required form-required form-item__label">Username</label>
///       <input autocorrect="none" autocapitalize="none" spellcheck="false" autofocus="autofocus" data-drupal-selector="edit-name" aria-describedby="edit-name--description" type="text" id="edit-name" name="name" value="" size="60" maxlength="60" class="form-text required form-item__textfield" required="required" aria-required="true" />
///       <div id="edit-name--description" class="form-item__description">
///         Your username.
///       </div>
///       <input autocomplete="off" data-drupal-selector="form-bhzme2hetuevnwqr5y4pyp8jcau2dfbherwoscwnajm" type="hidden" name="form_build_id" value="form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM" class="form-item__textfield" />
///       <input data-drupal-selector="edit-user-login-form" type="hidden" name="form_id" value="user_login_form" class="form-item__textfield" />
///       <div data-drupal-selector="edit-actions" class="form-actions js-form-wrapper form-wrapper" id="edit-actions"><input data-drupal-selector="edit-submit" type="submit" id="edit-submit" name="op" value="Log in" class="button js-form-submit form-submit form-item__textfield" />
///     </div>
///   </form>
/// </html>
/// "#;
///
/// let form = get_form(html, "user-login-form");
/// assert!(!form.is_empty());
/// ```
pub fn get_form(html: &str, name: &str) -> String {
    let re = Regex::new(&format!(
        // Lazy match to avoid matching multiple forms.
        r#"<form.*?data-drupal-selector="{}".*?>(.*?)</form>"#,
        name
    ))
    .unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire form, a subset of the received html.
    match re.captures(&line) {
        Some(capture) => capture[1].to_string(),
        None => {
            warn!("form {} not found", name);
            "".to_string()
        }
    }
}

/// Use regular expression to get the value of a named form element.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::{get_form, get_form_value};
///
/// // For this example we grab just a subset of a real Drupal form, enough to demonstrate. Normally
/// // you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr" class="light-mode">
///   <form class="user-login-form" data-drupal-selector="user-login-form" action=`/user/login` method="post" id="user-login-form" accept-charset="UTF-8">
///     <div class="js-form-item form-item">
///       <label for="edit-name" class="js-form-required form-required form-item__label">Username</label>
///       <input autocorrect="none" autocapitalize="none" spellcheck="false" autofocus="autofocus" data-drupal-selector="edit-name" aria-describedby="edit-name--description" type="text" id="edit-name" name="name" value="" size="60" maxlength="60" class="form-text required form-item__textfield" required="required" aria-required="true" />
///       <div id="edit-name--description" class="form-item__description">
///         Your username.
///       </div>
///       <input autocomplete="off" data-drupal-selector="form-bhzme2hetuevnwqr5y4pyp8jcau2dfbherwoscwnajm" type="hidden" name="form_build_id" value="form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM" class="form-item__textfield" />
///       <input data-drupal-selector="edit-form-token" type="hidden" name="form_token" value="5sM6gWNMbHoGq5RGKWQqSis3l5ulFkm4H8OG9pSIBw8" />
///       <input data-drupal-selector="edit-user-login-form" type="hidden" name="form_id" value="user_login_form" class="form-item__textfield" />
///       <div data-drupal-selector="edit-actions" class="form-actions js-form-wrapper form-wrapper" id="edit-actions"><input data-drupal-selector="edit-submit" type="submit" id="edit-submit" name="op" value="Log in" class="button js-form-submit form-submit form-item__textfield" />
///     </div>
///   </form>
/// </html>
/// "#;
///
/// let form = get_form(html, "user-login-form");
/// let form_build_id = get_form_value(&form, "form_build_id");
/// assert_eq!(&form_build_id, "form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM");
/// ```
pub fn get_form_value(form_html: &str, name: &str) -> String {
    let re = Regex::new(&format!(
        r#"name="{}" value=['"](.*?)['"]"#,
        regex::escape(name)
    ))
    .unwrap();
    // Return a specific form value.
    match re.captures(form_html) {
        Some(v) => v[1].to_string(),
        None => {
            warn!("form element {} not found", name);
            "none".to_string()
        }
    }
}

/// Use a regular expression to get a specific form that has been encoded to replace a BigPipe placeholder,
/// identified by the data-drupal-selector.
///
/// In Drupal 8.1+, BigPipe and Dynamic Page Cache can cause forms (and other content) to be replaced
/// with a placeholder where you'd normally expect it, and for an encoded version of the form to then
/// appear later in the same page html. This is a performance technique allowing the cacheable portions
/// of the page to be quickly visible to the end user. This function is similar to [`get_form`] but uses
/// an alternative regex to match an encoded form.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::get_bigpipe_form;
///
/// // For this example we grab just a subset of a real BigPipe placeholder form, enough to demonstrate.
/// // Normally you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr" class="light-mode">
/// <body>
///
/// <script type="application/vnd.drupal-ajax" data-big-pipe-event="start"></script>
/// <script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=shortcut.lazy_builders%3AlazyLinks&amp;&amp;token=N1997Wch59v-LxHku3-dD44wjkSNmhegNzlZ8jS0L5I">
/// [{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=shortcut.lazy_builders%3AlazyLinks\u0026\u0026token=N1997Wch59v-LxHku3-dD44wjkSNmhegNzlZ8jS0L5I\u0022]","data":"\u003Ca href=\u0022\/admin\/config\/user-interface\/shortcut\/manage\/default\/customize\u0022 class=\u0022edit-shortcuts\u0022\u003EEdit shortcuts\u003C\/a\u003E","settings":null}]
/// </script>    <script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=user.toolbar_link_builder%3ArenderDisplayName&amp;&amp;token=-MH2NzEnTzbzMk0ZGfGgoiw7G3j_-Q1ILWBRVhIOKLI">
/// [{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=user.toolbar_link_builder%3ArenderDisplayName\u0026\u0026token=-MH2NzEnTzbzMk0ZGfGgoiw7G3j_-Q1ILWBRVhIOKLI\u0022]","data":"admin","settings":null}]
/// </script>    <script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=Drupal%5CFormViewBuilder%3A%3AbuildForm&amp;args%5B0%5D=node&amp;args%5B1%5D=4&amp;args%5B2%5D=field_foo&amp;args%5B3%5D=fo&amp;token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc">
/// [{"command":"settings","settings":{"ajaxPageState":{"theme":"foo","libraries":"big_pipe\/big_pipe,blazy\/load,comment\/drupal.comment-by-viewer,devel\/devel-toolbar"}:{"edit-ajax-comments-reply-form-node-4-field-foo-0-0":{"url":"\/ajax_comments\/add\/node\/4\/field_foo","dialogType":"ajax","submit":{"_triggering_element_name":"op","_triggering_element_value":"Save"}}},"pluralDelimiter":"\u0003","user":{"uid":"1","permissionsHash":"0f3c5a3dcefdfd2cf26ca0b007b9d2610f88a9cdfa09b08220633755cc13f397"}},"merge":true},{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=Drupal%5CRender\u0026args%5B0%5D=node\u0026args%5B1%5D=4\u0026args%5B2%5D=field_foo\u0026args%5B3%5D=reviews\u0026token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc\u0022]","data":"\u003Cform class=\u0022comment-reviews-form comment-form ajax-comments-reply-form-node-4-field_foo-0-0 ajax-comments-form-add\u0022 id=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 data-drupal-selector=\u0022comment-form\u0022 action=\u0022\/comment\/reply\/node\/4\/field_foo\u0022 method=\u0022post\u0022 accept-charset=\u0022UTF-8\u0022\u003E\n  \u003Cdiv class=\u0022field--type-string field--name-subject field--widget-string-textfield js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-subject-wrapper\u0022 id=\u0022edit-subject-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-subject-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003ETitle\u003C\/label\u003E\n        \u003Cinput class=\u0022js-text-full text-full form-text required form-item__textfield\u0022 data-drupal-selector=\u0022edit-subject-0-value\u0022 type=\u0022text\u0022 id=\u0022edit-subject-0-value\u0022 name=\u0022subject[0][value]\u0022 value=\u0022\u0022 size=\u002260\u0022 maxlength=\u002264\u0022 placeholder=\u0022Give your review a title\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022 \/\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cinput data-drupal-selector=\u0022edit-form-html-id\u0022 type=\u0022hidden\u0022 name=\u0022form_html_id\u0022 value=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-wrapper-html-id\u0022 type=\u0022hidden\u0022 name=\u0022wrapper_html_id\u0022 value=\u0022node-foo-field-foo\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput autocomplete=\u0022off\u0022 data-drupal-selector=\u0022form-r8d9jop8ekobinr-vflozsd6erwor5-dhqx8s2tozly\u0022 type=\u0022hidden\u0022 name=\u0022form_build_id\u0022 value=\u0022form-R8d9JOp8eKObiNR_vFlOzSD6erWoR5-dHQx8s2toZLY\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form-form-token\u0022 type=\u0022hidden\u0022 name=\u0022form_token\u0022 value=\u00224OCYabXYY116z0_ixUaxzbYlVxEgchgThmF9O3uJqbI\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form\u0022 type=\u0022hidden\u0022 name=\u0022form_id\u0022 value=\u0022comment_reviews_form\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cdiv class=\u0022field--type-fivestar field--name-field-content-rating field--widget-fivestar-stars js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-field-content-rating-wrapper\u0022 id=\u0022edit-field-content-rating-wrapper\u0022\u003E      \u003Cdiv class=\u0022clearfix fivestar-none-text fivestar-average-stars fivestar-form-item fivestar-basic\u0022\u003E\u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-field-content-rating-0-rating\u0022 class=\u0022form-item__label\u0022\u003ERating\u003C\/label\u003E\n        \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n        \u003Cdiv class=\u0022form-item__dropdown\u0022\u003E\u003Cselect data-drupal-selector=\u0022edit-field-content-rating-0-rating\u0022 id=\u0022edit-field-content-rating-0-rating--2\u0022 name=\u0022field_content_rating[0][rating]\u0022 class=\u0022form-select form-item__select\u0022\u003E\u003Coption value=\u0022-\u0022\u003ESelect rating\u003C\/option\u003E\u003Coption value=\u002220\u0022\u003EGive it 1\/5\u003C\/option\u003E\u003Coption value=\u002240\u0022\u003EGive it 2\/5\u003C\/option\u003E\u003Coption value=\u002260\u0022\u003EGive it 3\/5\u003C\/option\u003E\u003Coption value=\u002280\u0022\u003EGive it 4\/5\u003C\/option\u003E\u003Coption value=\u0022100\u0022\u003EGive it 5\/5\u003C\/option\u003E\u003C\/select\u003E\u003C\/div\u003E\n        \u003C\/div\u003E\n\n        \u003C\/div\u003E\n\u003C\/div\u003E\n  \u003C\/div\u003E\n\u003Cdiv class=\u0022field--type-text-long field--name-comment-body field--widget-text-textarea js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-comment-body-wrapper\u0022 id=\u0022edit-comment-body-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-text-format-wrapper js-form-item form-item\u0022\u003E\n  \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-comment-body-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003EComment\u003C\/label\u003E\n        \u003Cdiv\u003E\n  \u003Ctextarea class=\u0022js-text-full text-full form-textarea required form-item__textfield form-item__textarea\u0022 data-media-embed-host-entity-langcode=\u0022en\u0022 data-drupal-selector=\u0022edit-comment-body-0-value\u0022 id=\u0022edit-comment-body-0-value\u0022 name=\u0022comment_body[0][value]\u0022 rows=\u00225\u0022 cols=\u002260\u0022 placeholder=\u0022Foo\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022\u003E\u003C\/textarea\u003E\n\u003C\/div\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cdiv data-drupal-selector=\u0022edit-actions\u0022 class=\u0022form-actions js-form-wrapper form-wrapper\u0022 id=\u0022edit-actions\u0022\u003E\u003Cinput data-drupal-selector=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 type=\u0022submit\u0022 id=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 name=\u0022op\u0022 value=\u0022Submit Review\u0022 class=\u0022button button--primary js-form-submit form-submit form-item__textfield\u0022 \/\u003E\n\u003C\/div\u003E\n\n\u003C\/form\u003E\n","settings":null}]
/// </script>    <script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=Drupal%5CFormViewBuilder%3A%3BbuildForm&amp;args%5B0%5D=node&amp;args%5B1%5D=4&amp;args%5B2%5D=field_bar&amp;args%5B3%5D=fo&amp;token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc">
/// [{"command":"settings","settings":{"ajaxPageState":{"theme":"bar","libraries":"big_pipe\/big_pipe,blazy\/load,comment\/drupal.comment-by-viewer,devel\/devel-toolbar"}:{"edit-ajax-comments-reply-form-node-4-field-bar-0-0":{"url":"\/ajax_comments\/add\/node\/4\/field_bar","dialogType":"ajax","submit":{"_triggering_element_name":"op","_triggering_element_value":"Save"}}},"pluralDelimiter":"\u0003","user":{"uid":"1","permissionsHash":"0f3c5a3dcefdfd2cf26ca0b007b9d2610f88a9cdfa09b08220633755cc13f397"}},"merge":true},{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=Drupal%5CRender\u0026args%5B0%5D=node\u0026args%5B1%5D=4\u0026args%5B2%5D=field_bar\u0026args%5B3%5D=reviews\u0026token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc\u0022]","data":"\u003Cform class=\u0022comment-reviews-form comment-form ajax-comments-reply-form-node-4-field_bar-0-0 ajax-comments-form-add\u0022 id=\u0022ajax-comments-reply-form-node-4-field-bar-0-0\u0022 data-drupal-selector=\u0022alternative-comment-form\u0022 action=\u0022\/comment\/reply\/node\/4\/field_bar\u0022 method=\u0022post\u0022 accept-charset=\u0022UTF-8\u0022\u003E\n  \u003Cdiv class=\u0022field--type-string field--name-subject field--widget-string-textfield js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-subject-wrapper\u0022 id=\u0022edit-subject-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-subject-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003ETitle\u003C\/label\u003E\n        \u003Cinput class=\u0022js-text-full text-full form-text required form-item__textfield\u0022 data-drupal-selector=\u0022edit-subject-0-value\u0022 type=\u0022text\u0022 id=\u0022edit-subject-0-value\u0022 name=\u0022subject[0][value]\u0022 value=\u0022\u0022 size=\u002260\u0022 maxlength=\u002264\u0022 placeholder=\u0022Give your review a title\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022 \/\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cinput data-drupal-selector=\u0022edit-form-html-id\u0022 type=\u0022hidden\u0022 name=\u0022form_html_id\u0022 value=\u0022ajax-comments-reply-form-node-4-field-bar-0-0\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-wrapper-html-id\u0022 type=\u0022hidden\u0022 name=\u0022wrapper_html_id\u0022 value=\u0022node-bar-field-bar\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput autocomplete=\u0022off\u0022 data-drupal-selector=\u0022form-r8d9jop8ekobinr-vflozsd6erwor5-dhqx8s2tozly\u0022 type=\u0022hidden\u0022 name=\u0022form_build_id\u0022 value=\u0022form-S7a8OJ2ebKObiNR_zRm3bW5EerWoR5-dHQx8s2toABC\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form-form-token\u0022 type=\u0022hidden\u0022 name=\u0022form_token\u0022 value=\u002281BDfbXZZ234a1_ixUaxzbYlVxTgchoInnA321jUrhU\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form\u0022 type=\u0022hidden\u0022 name=\u0022form_id\u0022 value=\u0022comment_reviews_form\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cdiv class=\u0022field--type-fivestar field--name-field-content-rating field--widget-fivestar-stars js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-field-content-rating-wrapper\u0022 id=\u0022edit-field-content-rating-wrapper\u0022\u003E      \u003Cdiv class=\u0022clearfix fivestar-none-text fivestar-average-stars fivestar-form-item fivestar-basic\u0022\u003E\u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-field-content-rating-0-rating\u0022 class=\u0022form-item__label\u0022\u003ERating\u003C\/label\u003E\n        \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n        \u003Cdiv class=\u0022form-item__dropdown\u0022\u003E\u003Cselect data-drupal-selector=\u0022edit-field-content-rating-0-rating\u0022 id=\u0022edit-field-content-rating-0-rating--2\u0022 name=\u0022field_content_rating[0][rating]\u0022 class=\u0022form-select form-item__select\u0022\u003E\u003Coption value=\u0022-\u0022\u003ESelect rating\u003C\/option\u003E\u003Coption value=\u002220\u0022\u003EGive it 1\/5\u003C\/option\u003E\u003Coption value=\u002240\u0022\u003EGive it 2\/5\u003C\/option\u003E\u003Coption value=\u002260\u0022\u003EGive it 3\/5\u003C\/option\u003E\u003Coption value=\u002280\u0022\u003EGive it 4\/5\u003C\/option\u003E\u003Coption value=\u0022100\u0022\u003EGive it 5\/5\u003C\/option\u003E\u003C\/select\u003E\u003C\/div\u003E\n        \u003C\/div\u003E\n\n        \u003C\/div\u003E\n\u003C\/div\u003E\n  \u003C\/div\u003E\n\u003Cdiv class=\u0022field--type-text-long field--name-comment-body field--widget-text-textarea js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-comment-body-wrapper\u0022 id=\u0022edit-comment-body-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-text-format-wrapper js-form-item form-item\u0022\u003E\n  \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-comment-body-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003EComment\u003C\/label\u003E\n        \u003Cdiv\u003E\n  \u003Ctextarea class=\u0022js-text-full text-full form-textarea required form-item__textfield form-item__textarea\u0022 data-media-embed-host-entity-langcode=\u0022en\u0022 data-drupal-selector=\u0022edit-comment-body-0-value\u0022 id=\u0022edit-comment-body-0-value\u0022 name=\u0022comment_body[0][value]\u0022 rows=\u00225\u0022 cols=\u002260\u0022 placeholder=\u0022Bar\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022\u003E\u003C\/textarea\u003E\n\u003C\/div\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cdiv data-drupal-selector=\u0022edit-actions\u0022 class=\u0022form-actions js-form-wrapper form-wrapper\u0022 id=\u0022edit-actions\u0022\u003E\u003Cinput data-drupal-selector=\u0022edit-ajax-comments-reply-form-node-4-field-bar-0-0\u0022 type=\u0022submit\u0022 id=\u0022edit-ajax-comments-reply-form-node-4-field-bar-0-0\u0022 name=\u0022op\u0022 value=\u0022Submit Review\u0022 class=\u0022button button--primary js-form-submit form-submit form-item__textfield\u0022 \/\u003E\n\u003C\/div\u003E\n\n\u003C\/form\u003E\n","settings":null}]
/// </script>
/// <script type="application/vnd.drupal-ajax" data-big-pipe-event="stop"></script>
/// </body>
/// </html>
/// "#;
///
/// let form = get_bigpipe_form(html, "comment-form");
/// // We matched at least one form.
/// assert!(!form.is_empty());
/// // We matched only one form.
/// assert_eq!(form.lines().count(), 1);
/// ```
pub fn get_bigpipe_form(html: &str, name: &str) -> String {
    let re = Regex::new(
        // Lazy match to avoid matching multiple forms.
        &format!(
            "{}.*?data-drupal-selector=.*?{}(.*?){}",
            regex::escape(r#"[{"#),
            regex::escape(name),
            regex::escape(r#"}]"#)
        ),
    )
    .unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire form, a subset of the received html.
    match re.captures(&line) {
        Some(capture) => capture[1].to_string(),
        None => {
            warn!("bigpipe form {} not found", name);
            "".to_string()
        }
    }
}

/// Load a form value from an encoded form.
///
/// Gets form values from forms that are returned by ajax callbacks or embedded by BigPipe.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::get_encoded_form_value;
///
/// let form = r#"<script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=Drupal%5CFormViewBuilder%3A%3AbuildForm&amp;args%5B0%5D=node&amp;args%5B1%5D=4&amp;args%5B2%5D=field_foo&amp;args%5B3%5D=fo&amp;token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc">
/// [{"command":"settings","settings":{"ajaxPageState":{"theme":"foo","libraries":"big_pipe\/big_pipe,blazy\/load,comment\/drupal.comment-by-viewer,devel\/devel-toolbar"}:{"edit-ajax-comments-reply-form-node-4-field-foo-0-0":{"url":"\/ajax_comments\/add\/node\/4\/field_foo","dialogType":"ajax","submit":{"_triggering_element_name":"op","_triggering_element_value":"Save"}}},"pluralDelimiter":"\u0003","user":{"uid":"1","permissionsHash":"0f3c5a3dcefdfd2cf26ca0b007b9d2610f88a9cdfa09b08220633755cc13f397"}},"merge":true},{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=Drupal%5CRender\u0026args%5B0%5D=node\u0026args%5B1%5D=4\u0026args%5B2%5D=field_foo\u0026args%5B3%5D=reviews\u0026token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc\u0022]","data":"\u003Cform class=\u0022comment-reviews-form comment-form ajax-comments-reply-form-node-4-field_foo-0-0 ajax-comments-form-add\u0022 id=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 data-drupal-selector=\u0022comment-form\u0022 action=\u0022\/comment\/reply\/node\/4\/field_foo\u0022 method=\u0022post\u0022 accept-charset=\u0022UTF-8\u0022\u003E\n  \u003Cdiv class=\u0022field--type-string field--name-subject field--widget-string-textfield js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-subject-wrapper\u0022 id=\u0022edit-subject-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-subject-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003ETitle\u003C\/label\u003E\n        \u003Cinput class=\u0022js-text-full text-full form-text required form-item__textfield\u0022 data-drupal-selector=\u0022edit-subject-0-value\u0022 type=\u0022text\u0022 id=\u0022edit-subject-0-value\u0022 name=\u0022subject[0][value]\u0022 value=\u0022\u0022 size=\u002260\u0022 maxlength=\u002264\u0022 placeholder=\u0022Give your review a title\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022 \/\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cinput data-drupal-selector=\u0022edit-form-html-id\u0022 type=\u0022hidden\u0022 name=\u0022form_html_id\u0022 value=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-wrapper-html-id\u0022 type=\u0022hidden\u0022 name=\u0022wrapper_html_id\u0022 value=\u0022node-foo-field-foo\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput autocomplete=\u0022off\u0022 data-drupal-selector=\u0022form-r8d9jop8ekobinr-vflozsd6erwor5-dhqx8s2tozly\u0022 type=\u0022hidden\u0022 name=\u0022form_build_id\u0022 value=\u0022form-R8d9JOp8eKObiNR_vFlOzSD6erWoR5-dHQx8s2toZLY\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form-form-token\u0022 type=\u0022hidden\u0022 name=\u0022form_token\u0022 value=\u00224OCYabXYY116z0_ixUaxzbYlVxEgchgThmF9O3uJqbI\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form\u0022 type=\u0022hidden\u0022 name=\u0022form_id\u0022 value=\u0022comment_reviews_form\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cdiv class=\u0022field--type-fivestar field--name-field-content-rating field--widget-fivestar-stars js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-field-content-rating-wrapper\u0022 id=\u0022edit-field-content-rating-wrapper\u0022\u003E      \u003Cdiv class=\u0022clearfix fivestar-none-text fivestar-average-stars fivestar-form-item fivestar-basic\u0022\u003E\u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-field-content-rating-0-rating\u0022 class=\u0022form-item__label\u0022\u003ERating\u003C\/label\u003E\n        \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n        \u003Cdiv class=\u0022form-item__dropdown\u0022\u003E\u003Cselect data-drupal-selector=\u0022edit-field-content-rating-0-rating\u0022 id=\u0022edit-field-content-rating-0-rating--2\u0022 name=\u0022field_content_rating[0][rating]\u0022 class=\u0022form-select form-item__select\u0022\u003E\u003Coption value=\u0022-\u0022\u003ESelect rating\u003C\/option\u003E\u003Coption value=\u002220\u0022\u003EGive it 1\/5\u003C\/option\u003E\u003Coption value=\u002240\u0022\u003EGive it 2\/5\u003C\/option\u003E\u003Coption value=\u002260\u0022\u003EGive it 3\/5\u003C\/option\u003E\u003Coption value=\u002280\u0022\u003EGive it 4\/5\u003C\/option\u003E\u003Coption value=\u0022100\u0022\u003EGive it 5\/5\u003C\/option\u003E\u003C\/select\u003E\u003C\/div\u003E\n        \u003C\/div\u003E\n\n        \u003C\/div\u003E\n\u003C\/div\u003E\n  \u003C\/div\u003E\n\u003Cdiv class=\u0022field--type-text-long field--name-comment-body field--widget-text-textarea js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-comment-body-wrapper\u0022 id=\u0022edit-comment-body-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-text-format-wrapper js-form-item form-item\u0022\u003E\n  \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-comment-body-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003EComment\u003C\/label\u003E\n        \u003Cdiv\u003E\n  \u003Ctextarea class=\u0022js-text-full text-full form-textarea required form-item__textfield form-item__textarea\u0022 data-media-embed-host-entity-langcode=\u0022en\u0022 data-drupal-selector=\u0022edit-comment-body-0-value\u0022 id=\u0022edit-comment-body-0-value\u0022 name=\u0022comment_body[0][value]\u0022 rows=\u00225\u0022 cols=\u002260\u0022 placeholder=\u0022Foo\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022\u003E\u003C\/textarea\u003E\n\u003C\/div\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cdiv data-drupal-selector=\u0022edit-actions\u0022 class=\u0022form-actions js-form-wrapper form-wrapper\u0022 id=\u0022edit-actions\u0022\u003E\u003Cinput data-drupal-selector=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 type=\u0022submit\u0022 id=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 name=\u0022op\u0022 value=\u0022Submit Review\u0022 class=\u0022button button--primary js-form-submit form-submit form-item__textfield\u0022 \/\u003E\n\u003C\/div\u003E\n\n\u003C\/form\u003E\n","settings":null}]
/// </script>"#;
///
/// let form_build_id = get_encoded_form_value(form, "form_build_id");
/// assert_eq!(form_build_id, "form-R8d9JOp8eKObiNR_vFlOzSD6erWoR5-dHQx8s2toZLY");
/// ```
pub fn get_encoded_form_value(form_html: &str, name: &str) -> String {
    // Decode quotes, which is enough for the normal get_form_value() regex to work.
    let decoded_form = form_html.replace("\\u0022", r#"""#);
    get_form_value(&decoded_form, name)
}

/// Extract an updated build_id from a form.
///
/// After certain form actions, such as uploading a file, Drupal can change the `build_id`. Requires the original
/// `build_id`.
///
/// ```rust
/// use goose_eggs::drupal::get_updated_build_id;
///
/// let build_id = "form-jsirb7DiRiBC09VrCJRfj-D1z6kjzX-sMqUgHmM_bCs";
///
/// let form_snippet = r#"{"command":"update_build_id","old":"form-jsirb7DiRiBC09VrCJRfj-D1z6kjzX-sMqUgHmM_bCs","new":"form-0VJ1MsfQR17RKlwarp_Rh_wMzbmjMlJc1SX_oPc0Bkc"}"#;
///
/// let updated_build_id = get_updated_build_id(form_snippet, build_id);
/// assert_eq!(updated_build_id, "form-0VJ1MsfQR17RKlwarp_Rh_wMzbmjMlJc1SX_oPc0Bkc");
/// ```
pub fn get_updated_build_id(form_html: &str, old_build_id: &str) -> String {
    let re = Regex::new(&format!(
        "{}{}{}",
        r#"\{"command":"update_build_id","old":""#, old_build_id, r#"","new":"(.*?)"\}"#
    ))
    .unwrap();
    // Return a specific form value.
    match re.captures(form_html) {
        Some(v) => v[1].to_string(),
        None => {
            warn!("update_build_id not found");
            "none".to_string()
        }
    }
}

/// Loop through an array of named form elements returning their values in a HashMap.
///
/// If loading values from an encoded form, use [`get_encoded_form_values`].
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::{get_form, get_form_values};
///
/// // For this example we grab just a subset of a real Drupal form, enough to demonstrate. Normally
/// // you'd use the entire html snippet returned from [`validate_and_load_static_assets`].
/// let html = r#"
/// <html lang="en" dir="ltr" class="light-mode">
///   <form class="user-login-form" data-drupal-selector="user-login-form" action=`/user/login` method="post" id="user-login-form" accept-charset="UTF-8">
///     <div class="js-form-item form-item">
///       <label for="edit-name" class="js-form-required form-required form-item__label">Username</label>
///       <input autocorrect="none" autocapitalize="none" spellcheck="false" autofocus="autofocus" data-drupal-selector="edit-name" aria-describedby="edit-name--description" type="text" id="edit-name" name="name" value="" size="60" maxlength="60" class="form-text required form-item__textfield" required="required" aria-required="true" />
///       <div id="edit-name--description" class="form-item__description">
///         Your username.
///       </div>
///       <input autocomplete="off" data-drupal-selector="form-bhzme2hetuevnwqr5y4pyp8jcau2dfbherwoscwnajm" type="hidden" name="form_build_id" value="form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM" class="form-item__textfield" />
///       <input data-drupal-selector="edit-form-token" type="hidden" name="form_token" value="5sM6gWNMbHoGq5RGKWQqSis3l5ulFkm4H8OG9pSIBw8" />
///       <input data-drupal-selector="edit-user-login-form" type="hidden" name="form_id" value="user_login_form" class="form-item__textfield" />
///       <div data-drupal-selector="edit-actions" class="form-actions js-form-wrapper form-wrapper" id="edit-actions"><input data-drupal-selector="edit-submit" type="submit" id="edit-submit" name="op" value="Log in" class="button js-form-submit form-submit form-item__textfield" />
///     </div>
///   </form>
/// </html>
/// "#;
///
/// let form = get_form(html, "user-login-form");
/// // Specify the three form elements we're looking for.
/// let form_values = get_form_values(&form, &["form_token", "form_build_id", "form_id"]);
/// // Confirm that all three form values were correctly identified.
/// assert_eq!(form_values.get("form_token").unwrap().as_str(), "5sM6gWNMbHoGq5RGKWQqSis3l5ulFkm4H8OG9pSIBw8");
/// assert_eq!(form_values.get("form_build_id").unwrap().as_str(), "form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM");
/// assert_eq!(form_values.get("form_id").unwrap().as_str(), "user_login_form");
/// ```
pub fn get_form_values<'a>(form: &str, elements: &'a [&str]) -> HashMap<&'a str, String> {
    let mut form_elements = HashMap::new();

    // Extract the form elements needed to submit a form.
    for &element in elements {
        let value = get_form_value(form, element);
        form_elements.insert(element, value);
    }

    form_elements
}

/// Loop through an array of named form elements loading them from an encoded form and
/// returning their values in a HashMap.
///
/// Gets form values from forms that are returned by ajax callbacks or embedded by BigPipe.
///
/// If loading values from a normal (non-encoded) form, use [`get_form_values`].
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::get_encoded_form_values;
///
/// let form = r#"<script type="application/vnd.drupal-ajax" data-big-pipe-replacement-for-placeholder-with-id="callback=Drupal%5CFormViewBuilder%3A%3AbuildForm&amp;args%5B0%5D=node&amp;args%5B1%5D=4&amp;args%5B2%5D=field_foo&amp;args%5B3%5D=fo&amp;token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc">
/// [{"command":"settings","settings":{"ajaxPageState":{"theme":"foo","libraries":"big_pipe\/big_pipe,blazy\/load,comment\/drupal.comment-by-viewer,devel\/devel-toolbar"}:{"edit-ajax-comments-reply-form-node-4-field-foo-0-0":{"url":"\/ajax_comments\/add\/node\/4\/field_foo","dialogType":"ajax","submit":{"_triggering_element_name":"op","_triggering_element_value":"Save"}}},"pluralDelimiter":"\u0003","user":{"uid":"1","permissionsHash":"0f3c5a3dcefdfd2cf26ca0b007b9d2610f88a9cdfa09b08220633755cc13f397"}},"merge":true},{"command":"insert","method":"replaceWith","selector":"[data-big-pipe-placeholder-id=\u0022callback=Drupal%5CRender\u0026args%5B0%5D=node\u0026args%5B1%5D=4\u0026args%5B2%5D=field_foo\u0026args%5B3%5D=reviews\u0026token=aru2saYxtVupc8Wt4DCKIB0JADknDRk2n1fS6OspTKc\u0022]","data":"\u003Cform class=\u0022comment-reviews-form comment-form ajax-comments-reply-form-node-4-field_foo-0-0 ajax-comments-form-add\u0022 id=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 data-drupal-selector=\u0022comment-form\u0022 action=\u0022\/comment\/reply\/node\/4\/field_foo\u0022 method=\u0022post\u0022 accept-charset=\u0022UTF-8\u0022\u003E\n  \u003Cdiv class=\u0022field--type-string field--name-subject field--widget-string-textfield js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-subject-wrapper\u0022 id=\u0022edit-subject-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-subject-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003ETitle\u003C\/label\u003E\n        \u003Cinput class=\u0022js-text-full text-full form-text required form-item__textfield\u0022 data-drupal-selector=\u0022edit-subject-0-value\u0022 type=\u0022text\u0022 id=\u0022edit-subject-0-value\u0022 name=\u0022subject[0][value]\u0022 value=\u0022\u0022 size=\u002260\u0022 maxlength=\u002264\u0022 placeholder=\u0022Give your review a title\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022 \/\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cinput data-drupal-selector=\u0022edit-form-html-id\u0022 type=\u0022hidden\u0022 name=\u0022form_html_id\u0022 value=\u0022ajax-comments-reply-form-node-4-field-foo-0-0\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-wrapper-html-id\u0022 type=\u0022hidden\u0022 name=\u0022wrapper_html_id\u0022 value=\u0022node-foo-field-foo\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput autocomplete=\u0022off\u0022 data-drupal-selector=\u0022form-r8d9jop8ekobinr-vflozsd6erwor5-dhqx8s2tozly\u0022 type=\u0022hidden\u0022 name=\u0022form_build_id\u0022 value=\u0022form-R8d9JOp8eKObiNR_vFlOzSD6erWoR5-dHQx8s2toZLY\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form-form-token\u0022 type=\u0022hidden\u0022 name=\u0022form_token\u0022 value=\u00224OCYabXYY116z0_ixUaxzbYlVxEgchgThmF9O3uJqbI\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cinput data-drupal-selector=\u0022edit-comment-reviews-form\u0022 type=\u0022hidden\u0022 name=\u0022form_id\u0022 value=\u0022comment_reviews_form\u0022 class=\u0022form-item__textfield\u0022 \/\u003E\n\u003Cdiv class=\u0022field--type-fivestar field--name-field-content-rating field--widget-fivestar-stars js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-field-content-rating-wrapper\u0022 id=\u0022edit-field-content-rating-wrapper\u0022\u003E      \u003Cdiv class=\u0022clearfix fivestar-none-text fivestar-average-stars fivestar-form-item fivestar-basic\u0022\u003E\u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-field-content-rating-0-rating\u0022 class=\u0022form-item__label\u0022\u003ERating\u003C\/label\u003E\n        \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n        \u003Cdiv class=\u0022form-item__dropdown\u0022\u003E\u003Cselect data-drupal-selector=\u0022edit-field-content-rating-0-rating\u0022 id=\u0022edit-field-content-rating-0-rating--2\u0022 name=\u0022field_content_rating[0][rating]\u0022 class=\u0022form-select form-item__select\u0022\u003E\u003Coption value=\u0022-\u0022\u003ESelect rating\u003C\/option\u003E\u003Coption value=\u002220\u0022\u003EGive it 1\/5\u003C\/option\u003E\u003Coption value=\u002240\u0022\u003EGive it 2\/5\u003C\/option\u003E\u003Coption value=\u002260\u0022\u003EGive it 3\/5\u003C\/option\u003E\u003Coption value=\u002280\u0022\u003EGive it 4\/5\u003C\/option\u003E\u003Coption value=\u0022100\u0022\u003EGive it 5\/5\u003C\/option\u003E\u003C\/select\u003E\u003C\/div\u003E\n        \u003C\/div\u003E\n\n        \u003C\/div\u003E\n\u003C\/div\u003E\n  \u003C\/div\u003E\n\u003Cdiv class=\u0022field--type-text-long field--name-comment-body field--widget-text-textarea js-form-wrapper form-wrapper\u0022 data-drupal-selector=\u0022edit-comment-body-wrapper\u0022 id=\u0022edit-comment-body-wrapper\u0022\u003E      \u003Cdiv class=\u0022js-text-format-wrapper js-form-item form-item\u0022\u003E\n  \u003Cdiv class=\u0022js-form-item form-item\u0022\u003E\n      \u003Clabel for=\u0022edit-comment-body-0-value\u0022 class=\u0022js-form-required form-required form-item__label\u0022\u003EComment\u003C\/label\u003E\n        \u003Cdiv\u003E\n  \u003Ctextarea class=\u0022js-text-full text-full form-textarea required form-item__textfield form-item__textarea\u0022 data-media-embed-host-entity-langcode=\u0022en\u0022 data-drupal-selector=\u0022edit-comment-body-0-value\u0022 id=\u0022edit-comment-body-0-value\u0022 name=\u0022comment_body[0][value]\u0022 rows=\u00225\u0022 cols=\u002260\u0022 placeholder=\u0022Foo\u0022 required=\u0022required\u0022 aria-required=\u0022true\u0022\u003E\u003C\/textarea\u003E\n\u003C\/div\u003E\n\n        \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\n  \u003C\/div\u003E\n\u003Cdiv data-drupal-selector=\u0022edit-actions\u0022 class=\u0022form-actions js-form-wrapper form-wrapper\u0022 id=\u0022edit-actions\u0022\u003E\u003Cinput data-drupal-selector=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 type=\u0022submit\u0022 id=\u0022edit-ajax-comments-reply-form-node-4-field-foo-0-0\u0022 name=\u0022op\u0022 value=\u0022Submit Review\u0022 class=\u0022button button--primary js-form-submit form-submit form-item__textfield\u0022 \/\u003E\n\u003C\/div\u003E\n\n\u003C\/form\u003E\n","settings":null}]
/// </script>"#;
///
/// // Specify the three form elements we're looking for.
/// let form_values = get_encoded_form_values(&form, &["form_token", "form_build_id", "form_id"]);
/// // Confirm that all three form values were correctly identified.
/// assert_eq!(form_values.get("form_token").unwrap().as_str(), "4OCYabXYY116z0_ixUaxzbYlVxEgchgThmF9O3uJqbI");
/// assert_eq!(form_values.get("form_build_id").unwrap().as_str(), "form-R8d9JOp8eKObiNR_vFlOzSD6erWoR5-dHQx8s2toZLY");
/// assert_eq!(form_values.get("form_id").unwrap().as_str(), "comment_reviews_form");
/// ```
pub fn get_encoded_form_values<'a>(form: &str, elements: &'a [&str]) -> HashMap<&'a str, String> {
    // Decode quotes one time, which is enough for the normal get_form_value() regex
    // to work.
    let decoded_form = form.replace("\\u0022", r#"""#);

    // Create a HashMap to return all request form_values.
    let mut form_elements = HashMap::new();

    // Extract the form elements needed to submit a form.
    for &element in elements {
        let value = get_form_value(&decoded_form, element);
        form_elements.insert(element, value);
    }

    form_elements
}

/// Set one or more defaults when logging in through the standard drupal user-login-form.
///
/// This object is passed to [`log_in`] to set a custom default username and/or password
/// and/or log in url and/or the required title after login.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::Login;
///
/// fn examples() {
///     // Manually build a Login structure with custom username and password.
///     let _login = Login::new(Some("foo"), Some("bar"), None, None);
///
///     // Call `Login::username_password` to build the same.
///     let mut login = Login::username_password("foo", "bar");
///
///     // Now also change the url and expected title.
///     login.unwrap().update_url_title("/custom/user/login", "Custom title");
/// }
pub struct Login<'a> {
    // Optionally set a default username.
    username: Option<&'a str>,
    // Optionally set a default password.
    password: Option<&'a str>,
    // Optionally set a custom default path (otherwise defaults to `/user/login`).
    url: Option<&'a str>,
    // Optionally set a custom title to validate.
    title: Option<&'a str>,
}
impl<'a> Login<'a> {
    /// Create a new Login object, specifying `username`, `password`, `url`, and expected
    /// `title`.
    ///
    /// It's generally preferred to use a helper such as [`Login::username_password`] or
    /// [`Login::url_title`] instead of invoking this function directly.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::new(
    ///     // Set a default username of "foo".
    ///     Some("foo"),
    ///     // Set a default password of "bar".
    ///     Some("bar"),
    ///     // Set a custom log-in path of "/custom/login/path".
    ///     Some("/custom/login/path"),
    ///     // Set a custom title to validate after log-in.
    ///     Some("Custom Title"),
    /// );
    /// ```
    pub fn new(
        username: Option<&'a str>,
        password: Option<&'a str>,
        url: Option<&'a str>,
        title: Option<&'a str>,
    ) -> Option<Login<'a>> {
        Some(Login {
            username,
            password,
            url,
            title,
        })
    }

    /// Create a Login object setting a custom default username.
    ///
    /// The password will remain the default of `password`. The login url will remain the
    /// default of `/user/login`. After login the title will be validated to confirm it
    /// include's the username. The username and password defaults can still be overridden
    /// by the `GOOSE_USER` and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username("foo");
    /// ```
    pub fn username(username: &'a str) -> Option<Login<'a>> {
        Login::new(Some(username), None, None, None)
    }

    /// Create a Login object setting a custom default password.
    ///
    /// The username will remain the default of `username`. The login url will remain the
    /// default of `/user/login`. After login the title will be validated to confirm it
    /// include's the username. The username and password defaults can still be overridden
    /// by the `GOOSE_USER` and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password("bar");
    /// ```
    pub fn password(password: &'a str) -> Option<Login<'a>> {
        Login::new(None, Some(password), None, None)
    }

    /// Create a Login object setting a custom default username and password.
    ///
    /// The login url will remain the default of `/user/login`. After login the title will
    /// be validated to confirm it include's the username. The username and password defaults
    /// can still be overridden by the `GOOSE_USER` and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username_password("foo", "bar");
    /// ```
    pub fn username_password(username: &'a str, password: &'a str) -> Option<Login<'a>> {
        Login::new(Some(username), Some(password), None, None)
    }

    /// Create a Login object with a custom default login url.
    ///
    /// The username will remain the default of `username`. The password will remain the
    /// default of `password`. After login the title will be validated to confirm it
    /// include's the username. The username and password defaults can still be
    /// overridden by the `GOOSE_USER` and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password("bar");
    /// ```
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::url("/custom/user/login");
    /// ```
    pub fn url(url: &'a str) -> Option<Login<'a>> {
        Login::new(None, None, Some(url), None)
    }

    /// Create a Login object with a custom expected title after login.
    ///
    /// The username will remain the default of `username`. The password will remain the
    /// default of `password`. The login url will remain the default of `/user/login`.
    /// The username and password defaults can still be overridden by the `GOOSE_USER` and
    /// `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password("bar");
    /// ```
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::title("Custom title");
    /// ```
    pub fn title(title: &'a str) -> Option<Login<'a>> {
        Login::new(None, None, None, Some(title))
    }

    /// Create a Login object with custom default url and a custom expected title after
    /// login.
    ///
    /// The username will remain the default of `username`. The password will remain the
    /// default of `password`. The username and password defaults can still be overridden
    /// by the `GOOSE_USER` and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password("bar");
    /// ```
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::url_title("/custom/login/path", "Custom title");
    /// ```
    pub fn url_title(url: &'a str, title: &'a str) -> Option<Login<'a>> {
        Login::new(None, None, Some(url), Some(title))
    }

    /// Update a Login object, changing the default username.
    ///
    /// The password, url and title fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::password("bar")
    ///         .unwrap()
    ///         .update_username("foo");
    /// ```
    pub fn update_username(mut self, username: &'a str) -> Option<Self> {
        self.username = Some(username);
        Some(self)
    }

    /// Update a Login object, changing the default password.
    ///
    /// The username, url and title fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::username("foo")
    ///         .unwrap()
    ///         .update_password("bar");
    /// ```
    pub fn update_password(mut self, password: &'a str) -> Option<Self> {
        self.password = Some(password);
        Some(self)
    }

    /// Update a Login object, changing the default username and password.
    ///
    /// The url and title fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::username_password("foo", "bar")
    ///         .unwrap()
    ///         .update_username_password("changed-username", "changed-password");
    /// ```
    pub fn update_username_password(
        mut self,
        username: &'a str,
        password: &'a str,
    ) -> Option<Self> {
        self.username = Some(username);
        self.password = Some(password);
        Some(self)
    }

    /// Update a Login object, changing the default login url.
    ///
    /// The username, password and title fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::username("foo")
    ///         .unwrap()
    ///         .update_url("/custom/user/login");
    /// ```
    pub fn update_url(mut self, url: &'a str) -> Option<Self> {
        self.url = Some(url);
        Some(self)
    }

    /// Update a Login object, changing the expected title after login.
    ///
    /// The username and password fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::username("foo")
    ///         .unwrap()
    ///         .update_title("Custom Title");
    /// ```
    pub fn update_title(mut self, title: &'a str) -> Option<Self> {
        self.title = Some(title);
        Some(self)
    }

    /// Update a Login object, changing the default login url and the expected title
    /// after login.
    ///
    /// The username and password fields will not be changed.
    ///
    /// The username and password defaults can still be overridden by the `GOOSE_USER`
    /// and `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let login =
    ///     Login::username_password("foo", "password")
    ///         .unwrap()
    ///         .update_url_title("/custom/user/login", "Custom Title");
    /// ```
    pub fn update_url_title(mut self, url: &'a str, title: &'a str) -> Option<Self> {
        self.url = Some(url);
        self.title = Some(title);
        Some(self)
    }
}

/// Log into a Drupal website.
///
/// The reference to a GooseUser object is from a Goose task function. The optional
/// pointer to a [`Login`] object can be created to override the username, password,
/// login url, or expected page title after log in.
///
/// If no default username is set in the [`Login`] object, the function will default to
/// a username of `username` which can be overridden by the `GOOSE_USER` environment variable.
/// If no default password is set in the [`Login`] object, the function will default to
/// a password of `password` which can be overridden by the `GOOSE_PASS` environment variable.
/// If no default url is set in the [`Login`] object, the function will default to a url
/// of `/user/login`. If no default title is set in the [`Login`] object, the function
/// will verify that the title includes the username after login.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::drupal::{log_in, Login};
///
/// task!(login).set_on_start();
///
/// async fn login(user: &GooseUser) -> GooseTaskResult {
///     // By default log in with `foo`:`bar`.
///     let _html = log_in(&user, Login::username_password("foo", "bar").as_ref()).await?;
///
///     Ok(())
/// }
///
/// ```
pub async fn log_in(user: &GooseUser, login: Option<&Login<'_>>) -> Result<String, GooseTaskError> {
    // Use the `GOOSE_USER` environment variable if it's set, otherwise use the custom username
    // passed in when calling this function, otherwise use `username`.
    let default_password = "username";
    let username = env::var("GOOSE_USER").unwrap_or_else(|_| match login {
        Some(l) => l.username.unwrap_or(default_password).to_string(),
        None => default_password.to_string(),
    });
    // Use the `GOOSE_PASS` environment variable if it's set, otherwise use the custom password
    // passed in when calling this function, otherwise use `password`.
    let default_password = "password";
    let password = env::var("GOOSE_PASS").unwrap_or_else(|_| match login {
        Some(l) => l.password.unwrap_or(default_password).to_string(),
        None => default_password.to_string(),
    });

    // Load the log in page.
    let default_login = "/user/login";
    let login_url = match login {
        Some(l) => l.url.unwrap_or(default_login),
        None => default_login,
    };
    let goose = user.get(login_url).await?;

    // Save the request to extract the form_build_id.
    let mut login_request = goose.request.clone();
    let login_page = crate::validate_and_load_static_assets(
        user,
        goose,
        &crate::Validate::text(r#"<form class="user-login-form"#),
    )
    .await?;

    // A web page can have multiple forms, so first get the correct form.
    let login_form = get_form(&login_page, "user-login-form");
    if login_form.is_empty() {
        user.set_failure(
            &format!("{}: no user-login-form on page", login_url),
            &mut login_request,
            None,
            Some(&login_page),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok("".to_string());
    }

    // Now extract the form_build_id in order to POST to the log in form.
    let form_build_id = get_form_value(&login_form, "form_build_id");
    if form_build_id.is_empty() {
        user.set_failure(
            &format!("{}: no form_build_id on page", login_url),
            &mut login_request,
            None,
            Some(&login_form),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok("".to_string());
    }

    // Build log in form with username and password from environment.
    let params = [
        ("name", &username),
        ("pass", &password),
        ("form_build_id", &form_build_id),
        ("form_id", &"user_login_form".to_string()),
        ("op", &"Log+in".to_string()),
    ];
    let request_builder = user.goose_post("/user/login").await?;
    let mut logged_in_user = user.goose_send(request_builder.form(&params), None).await?;

    // A successful log in is redirected.
    if !logged_in_user.request.redirected {
        // There was an error, get the headers and html if any to aid in debugging.
        let headers;
        let html = match logged_in_user.response {
            Ok(r) => {
                headers = Some(r.headers().clone());
                r.text().await.unwrap_or_else(|e| e.to_string())
            }
            Err(e) => {
                headers = None;
                e.to_string()
            }
        };
        user.set_failure(
            &format!(
                "{}: login failed (check `GOOSE_USER` and `GOOSE_PASS`)",
                logged_in_user.request.final_url
            ),
            &mut logged_in_user.request,
            headers.as_ref(),
            Some(&html),
        )?;
        // Return the html that was loaded, even though log-in failed. Enable
        // the debug_log to determine why log-in failed.
        return Ok(html);
    }

    // By default expect the username to be in the title.
    let default_title = username;
    let title = match login {
        // Allow a different expected title than the Drupal default.
        Some(l) => l.title.unwrap_or(&default_title),
        None => &default_title,
    };

    // Check the title to verify that the user is actually logged in.
    let logged_in_page = crate::validate_and_load_static_assets(
        user,
        logged_in_user,
        &crate::Validate::title(title),
    )
    .await?;

    Ok(logged_in_page)
}

/// Set parameters for making and validating a search.
pub struct SearchParams<'a> {
    // The word or words to search for.
    //
    // Defaults to `""`, an empty string.
    keys: Option<&'a str>,
    // Optionally set a custom path to the search form.
    //
    // Defaults to `search`.
    url: Option<&'a str>,
    // Optionally set a custom `op` name for the search button.
    //
    // Defaults to `Search`.
    submit: Option<&'a str>,
    // Optionally validate the title of the search form page.
    //
    // Defaults to None.
    title: Option<&'a str>,
}
impl<'a> SearchParams<'a> {
    /// Create a new [`SearchParams`] object, specifying `keys`, `url`, the name of the
    /// `submit` button, and the `title` of the search page. This object is passed to
    /// the [`search`] function.
    ///
    /// It is recommended to use a helper such as [`SearchParams::keys`] together with
    /// [`SearchParams::update_url`], [`SearchParams::update_submit`], and/or
    /// [`SearchParams::update_title`] instead of invoking this function directly.
    ///
    ///  # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Search for "search terms".
    /// let search = SearchParams::new(Some("search terms"), Some("custom/search/path"), Some("Custom Search"), Some("Custom Search"));
    /// ```
    pub fn new(
        keys: Option<&'a str>,
        url: Option<&'a str>,
        submit: Option<&'a str>,
        title: Option<&'a str>,
    ) -> SearchParams<'a> {
        SearchParams {
            keys,
            url,
            submit,
            title,
        }
    }

    /// Create a [`SearchParams`] object setting the string to search for.
    ///
    /// This object is passed to the [`search`] function.
    ///
    /// The search form url will remain the default of `search`, and can be changed by
    /// invoking [`SearchParams::update_url`]. The search form submit button will remain
    /// the default of `Search`, and can be changed by invoking
    /// [`SearchParams::update_submit`]
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Search for "search terms".
    /// let search = SearchParams::keys("search terms");
    /// ```
    pub fn keys(keys: &'a str) -> SearchParams<'a> {
        SearchParams::new(None, Some(keys), None, None)
    }

    /// Modify a [`SearchParams`] object setting a custom url for the search form.
    ///
    /// This object is passed to the [`search`] function.
    ///
    /// The [`SearchParams`] object is created by calling [`SearchParams::keys`], then
    /// it is chained to this function to set a custom url. The search form submit
    /// button will remain the default of `Search`, or whatever is set by invoking
    /// [`SearchParams::update_submit`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Search for "search terms" using a form with a custom path.
    /// let search = SearchParams::keys("search terms")
    ///                  .update_url("custom/search/path");
    /// ```
    pub fn update_url(mut self, url: &'a str) -> Self {
        self.url = Some(url);
        self
    }

    /// Modify a [`SearchParams`] object setting a custom submit button for the search
    /// form.
    ///
    /// This object is passed to the [`search`] function.
    ///
    /// The [`SearchParams`] object is created by calling [`SearchParams::keys`], then
    /// it is chained to this function to set a custom submit button. The search form
    /// url will remain the default of `search`, or whatever is set by invoking
    /// [`SearchParams::update_url`].
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Search for "search terms" with a form that has a custom submit botton.
    /// let search = SearchParams::keys("search terms")
    ///                  .update_submit("Custom Search");
    /// ```
    pub fn update_submit(mut self, submit: &'a str) -> Self {
        self.submit = Some(submit);
        self
    }

    /// Modify a [`SearchParams`] object to validate the title of the search form page.
    ///
    /// This object is passed to the [`search`] function.
    ///
    /// The [`SearchParams`] object is created by calling [`SearchParams::keys`], then
    /// it is chained to this function to validate the search form page title.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Search for "search terms" with a form on a page with title "Search".
    /// let search = SearchParams::keys("search terms")
    ///                  .update_title("Search");
    /// ```
    pub fn update_title(mut self, title: &'a str) -> Self {
        if title.is_empty() {
            self.title = None;
        } else {
            self.title = Some(title);
        }
        self
    }

    /// Modify a [`SearchParams`] object changing the string that is searched for.
    ///
    /// This object is passed to the [`search`] function.
    ///
    /// This function allows a mutable [`SearchParams`] object to be created with a
    /// custom url and/or submit button and then to be used for multiple different
    /// searches.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // First search for "search terms" using a form on a custom path.
    /// let mut search = SearchParams::keys("search terms")
    ///                  .update_url("custom/search/path");
    ///
    /// // Perform a search here ...
    ///
    /// // Then search for "different search terms" at the same custom path.
    /// search.update_keys("different search terms");
    /// ```
    pub fn update_keys(mut self, keys: &'a str) -> Self {
        self.keys = Some(keys);
        self
    }
}

/// Perform a simple Drupal-powered search.
///
/// In the following example, [`SearchParams::keys`] is used to configure the keys
/// being searched for, and [`SearchParams::update_title`] is used to validate that
/// the page with the search form has a title containing `Search`.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
///
/// task!(search);
///
/// async fn search(user: &GooseUser) -> GooseTaskResult {
///     // Use the default search form to search for "foo", validating that the
///     // search page has a title of Search.
///     let search_params = goose_eggs::drupal::SearchParams::keys("foo").update_title("Search");
///     // Perform the actual search.
///     let _search_results = goose_eggs::drupal::search(&user, &search_params).await?;
///
///     Ok(())
/// }
/// ```
pub async fn search<'a>(
    user: &GooseUser,
    params: &'a SearchParams<'a>,
) -> Result<String, GooseTaskError> {
    // Set default url if it isn't set in SearchParams.
    let url = if let Some(url) = params.url {
        url
    } else {
        "search"
    };

    // Load the search page.
    let goose = user.get(url).await?;

    // Optionally validate the title of the page with the search form.
    let validate = if let Some(title) = params.title {
        crate::Validate::title(title)
    } else {
        crate::Validate::none()
    };
    let search_page = crate::validate_and_load_static_assets(user, goose, &validate).await?;

    // Extract the search form from the page.
    let search_form = get_form(&search_page, "search-form");

    // Extract the form_build_id and the form_id from the search form.
    let form_values = get_form_values(&search_form, &["form_build_id", "form_id"]);

    // Perform empty search if not set in Search Params.
    let keys = if let Some(keys) = params.keys {
        keys
    } else {
        ""
    };

    // By default Drupal names the submit button "Search".
    let submit = if let Some(submit) = params.submit {
        submit
    } else {
        "Search"
    };

    // Build search form.
    let params = [
        ("form_build_id", form_values.get("form_build_id").unwrap()),
        ("form_id", form_values.get("form_id").unwrap()),
        ("keys", &keys.to_string()),
        ("op", &submit.to_string()),
    ];

    // Perform the search.
    let request_builder = user.goose_post(url).await?;
    let goose = user.goose_send(request_builder.form(&params), None).await?;

    // Validate that a search was performed, and the search keys are in the title.
    let validate = crate::Validate::title(keys);
    let search_results = crate::validate_and_load_static_assets(user, goose, &validate).await?;

    // Return the search results.
    Ok(search_results)
}
