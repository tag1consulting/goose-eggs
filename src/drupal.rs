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
        r#"<form.*?(data-drupal-selector|id)="{name}".*?>(.*?)</form>"#,
    ))
    .unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace('\n', "");
    // Return the entire form, a subset of the received html.
    match re.captures(&line) {
        Some(capture) => capture[2].to_string(),
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
    let line = html.replace('\n', "");
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

/// Parameters that define how to log into a Drupal website and validate
/// that the user loged in successfully. For complete documentation, refer
/// to [`LoginBuilder`].
#[derive(Clone, Debug)]
pub struct Login<'a> {
    // Optionally set a default username.
    username: &'a str,
    // Optionally set a default password.
    password: &'a str,
    // Optionally set a custom log in path (otherwise defaults to
    // `/user/login`).
    url: &'a str,
    // Optionally set a custom title to validate.
    log_in_page_validation: Option<&'a crate::Validate<'a>>,
    // Optionally set a custom title to validate.
    logged_in_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> Login<'a> {
    /// Convenience function to bring [`LoginBuilder`] into scope.
    pub fn builder() -> LoginBuilder<'a> {
        LoginBuilder::new()
    }
}

/// Used to build a [`Login`] object, necessary to invoke the [`log_in`] function.
///
/// Sets parameters that define how to log into a Drupal website and validate that
/// the user loged in successfully.
///
/// This object is passed to the [`log_in`] function.
///
/// The defined `username` and/or `password` can be dynamically overridden by setting
/// the `GOOSE_USER` and/or `GOOSE_PASS` environment variables when starting the load
/// test.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::Login;
///
/// // Manually build a Login structure with custom default username and password,
/// // and a custom log in path.
/// let _login = Login::builder()
///     .username("foo")
///     .password("bar")
///     .url("custom/user/login")
///     .build();
/// # mod support { include!("../tests/doc/support.rs"); }
/// # let server = support::log_in_server("foo", "bar", "custom/user/login");
/// # support::log_in("LoginBuilder", &server, &_login);
/// ```
pub struct LoginBuilder<'a> {
    // Optionally set a default username.
    username: &'a str,
    // Optionally set a default password.
    password: &'a str,
    // Optionally set a custom default path (otherwise defaults to `/user/login`).
    url: &'a str,
    // Optionally perform validation of the page with the login form.
    log_in_page_validation: Option<&'a crate::Validate<'a>>,
    // Optionally perform validation once the user logs in.
    logged_in_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> LoginBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to a username of "username".
            username: "username",
            // Defaults to a password of "password".
            password: "password",
            // Defaults to Drupal's standard login path of "user/login".
            url: "user/login",
            // Default tos performing no extra validation.
            log_in_page_validation: None,
            // Defaults to performing no extra validation.
            logged_in_page_validation: None,
        }
    }

    /// Used with [`Login::builder`] to configure login parameters.
    ///
    /// Defaults to `username`.
    ///
    /// Once built, the resulting object is passed to the [`log_in`] function. The
    /// username and password can still be overridden by the `GOOSE_USER` and
    /// `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// // Login with username "foo", and the default password of "password".
    /// let _login = Login::builder()
    ///     .username("foo")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("foo", "password", "user/login");
    /// # support::log_in("username", &server, &_login);
    /// ```
    pub fn username(mut self, username: impl Into<&'a str>) -> Self {
        self.username = username.into();
        self
    }

    /// Used with [`Login::builder`] to configure login parameters.
    ///
    /// Defaults to `password`.
    ///
    /// Once built, the resulting object is passed to the [`log_in`] function. The
    /// username and password can still be overridden by the `GOOSE_USER` and
    /// `GOOSE_PASS` environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// // Login with username "bar", and the default username of "username".
    /// let _login = Login::builder()
    ///     .password("bar")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("username", "bar", "user/login");
    /// # support::log_in("password", &server, &_login);
    /// ```
    pub fn password(mut self, password: impl Into<&'a str>) -> Self {
        self.password = password.into();
        self
    }

    /// Used with [`Login::builder`] to configure login parameters.
    ///
    /// Defaults to `user/login`.
    ///
    /// Once built, the resulting object is passed to the [`log_in`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// // Login at a custom path.
    /// let _login = Login::builder()
    ///     .url("custom/user/login")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("username", "password", "custom/user/login");
    /// # support::log_in("url", &server, &_login);
    /// ```
    pub fn url(mut self, url: impl Into<&'a str>) -> Self {
        self.url = url.into();
        self
    }

    /// Used with [`Login::builder`] to tell the [`log_in`] function to perform extra
    /// validation of the page containing the log in form.
    ///
    /// Defaults to `None`, so no extra validation is performed. By default it will still
    /// validate that `<form class="user-login-form"` exists on the log in page, and it
    /// will load all static assets on the page with the log in form.
    ///
    /// What validation should be performed is defined by passing a reference to a
    /// [`Validate`](../struct.Validate.html) object.
    ///
    /// Once built, the resulting object is passed to the [`log_in`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::{drupal, Validate};
    ///
    /// // This validation is done by default.
    /// let validate = Validate::builder()
    ///     .text(r#"<form class="user-login-form"#)
    ///     .build();
    /// let _login = drupal::Login::builder()
    ///     .log_in_page_validation(&validate)
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("username", "password", "user/login");
    /// # support::log_in("log_in_page_validation", &server, &_login);
    /// ```
    pub fn log_in_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.log_in_page_validation = Some(validation);
        self
    }

    /// Used with [`Login::builder`] to tell the [`log_in`] function to perform extra
    /// validation of the page returned once the user logs in.
    ///
    /// Defaults to `None`, so no extra validation is performed. By default it will still
    /// validate that the username of the user that logged in is in the title, and will
    /// load all static assets on the returned page.
    ///
    /// What validation should be performed is defined by passing a reference to a
    /// [`Validate`](../struct.Validate.html) object.
    ///
    /// Once built, the resulting object is passed to the [`log_in`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Use Drupal's default search form to search for "foo bar".
    /// let search_params = SearchParams::builder()
    ///     .keys("foo bar")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("keys", "foo bar")], "Results");
    /// # support::search("logged_in_page_validation, search", &server, &search_params);
    /// ```
    /// # Example
    /// ```rust
    /// use goose_eggs::{drupal, Validate};
    ///
    /// let username = "foo";
    ///
    /// // This validation is done by default.
    /// let validate = Validate::builder()
    ///     .title(username)
    ///     .build();
    /// let _login = drupal::Login::builder()
    ///     .username(username)
    ///     .logged_in_page_validation(&validate)
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("foo", "password", "user/login");
    /// # support::log_in("logged_in_page_validation", &server, &_login);
    /// ```
    pub fn logged_in_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.logged_in_page_validation = Some(validation);
        self
    }

    /// Build the [`Login`] object which is then passed to the [`log_in`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// // Log in as `foo` with the default password.
    /// let _login = Login::builder()
    ///     .username("foo")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::log_in_server("foo", "password", "user/login");
    /// # support::log_in("LoginBuilder::build", &server, &_login);
    /// ```
    pub fn build(self) -> Login<'a> {
        let Self {
            username,
            password,
            url,
            log_in_page_validation,
            logged_in_page_validation,
        } = self;
        Login {
            username,
            password,
            url,
            log_in_page_validation,
            logged_in_page_validation,
        }
    }
}

/// Log into a Drupal website.
///
/// The reference to a GooseUser object is from a transaction function. The optional
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
/// transaction!(login).set_on_start();
///
/// async fn login(user: &mut GooseUser) -> TransactionResult {
///     // By default log in with `foo`:`bar`.
///     let login = Login::builder()
///         .username("foo")
///         .password("bar")
///         .build();
///     let _html = log_in(user, &login).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/support.rs"); }
/// # let server = support::log_in_server("foo", "bar", "user/login");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(login(&mut user));
/// # assert!(result.is_ok(), "doc example log_in: {}", support::failure(&result));
///
/// ```
pub async fn log_in(
    user: &mut GooseUser,
    login: &Login<'_>,
) -> Result<String, Box<TransactionError>> {
    // Use the `GOOSE_USER` environment variable if it's set, otherwise use the specified
    // (or default) login username.
    let username = env::var("GOOSE_USER").unwrap_or_else(|_| login.username.to_string());

    // Use the `GOOSE_PASS` environment variable if it's set, otherwise use the specified
    // (or default) login password.
    let password = env::var("GOOSE_PASS").unwrap_or_else(|_| login.password.to_string());

    // By default verify that the standard user-login-form exists on the page.
    let default_validation = crate::Validate::builder()
        .text(r#"<form class="user-login-form"#)
        .build();
    let validate = if let Some(validation) = login.log_in_page_validation {
        validation
    } else {
        &default_validation
    };

    // Load the log in page.
    let goose = if let Some(validate_status) = validate.status.as_ref() {
        // Build request manually if validating a specific status code.
        let goose_request = GooseRequest::builder()
            .path(login.url)
            .expect_status_code(validate_status.status_code)
            .build();
        user.request(goose_request).await.unwrap()
    } else {
        // Otherwise follow default validation rules for status codes.
        user.get(login.url).await.unwrap()
    };

    let mut login_request = goose.request.clone();
    let login_page = crate::validate_and_load_static_assets(user, goose, validate).await?;

    // A web page can have multiple forms, so first get the correct form.
    let login_form = get_form(&login_page, "user-login-form");
    if login_form.is_empty() {
        user.set_failure(
            &format!("{}: no user-login-form on page", login.url),
            &mut login_request,
            None,
            Some(&login_page),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok("".to_string());
    }

    // Now extract the form_build_id in order to POST to the log in form
    // (`get_form_value` returns "none" when it is missing).
    let form_build_id = get_form_value(&login_form, "form_build_id");
    if form_build_id.is_empty() || form_build_id == "none" {
        user.set_failure(
            &format!("{}: no form_build_id on page", login.url),
            &mut login_request,
            None,
            Some(&login_form),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok("".to_string());
    }

    // Also extract the form_id (defaults to `user_login_form`).
    let form_id = get_form_value(&login_form, "form_id");
    if form_id.is_empty() || form_id == "none" {
        user.set_failure(
            &format!("{}: no form_id on page", login.url),
            &mut login_request,
            None,
            Some(&login_form),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok("".to_string());
    }

    // By default verify that the username that logged in is in the title of the
    // logged in page.
    let default_validation = crate::Validate::builder().title(username.as_str()).build();
    let validate = if let Some(validation) = login.logged_in_page_validation {
        validation
    } else {
        &default_validation
    };

    // Build log in form with username and password from environment.
    let params = [
        ("name", &username),
        ("pass", &password),
        ("form_build_id", &form_build_id),
        ("form_id", &form_id),
        ("op", &"Log in".to_string()),
    ];
    // Post the log in form.
    let mut logged_in_user = if let Some(validate_status) = validate.status.as_ref() {
        // Build request manually if validating a specific status code.
        let url = user.build_url(login.url)?;
        // A request builder object is necessary to post a form.
        let reqwest_request_builder = user.client.post(&url);
        let goose_request = GooseRequest::builder()
            .path(login.url)
            .method(GooseMethod::Post)
            .expect_status_code(validate_status.status_code)
            .set_request_builder(reqwest_request_builder.form(&params))
            .build();
        user.request(goose_request).await.unwrap()
    } else {
        user.post_form(login.url, &params).await?
    };

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

    // Check the title to verify that the user is actually logged in.
    let logged_in_page =
        crate::validate_and_load_static_assets(user, logged_in_user, validate).await?;

    Ok(logged_in_page)
}

/// Parameters that define how to make and validate a search. For complete documentation,
/// refer to [`SearchParamsBuilder`].
#[derive(Clone, Debug)]
pub struct SearchParams<'a> {
    // The word or words to search for.
    //
    // Default to `""`, an empty String.
    keys: &'a str,
    // Optionally set a custom path to the search form.
    //
    // Defaults to `search`.
    url: &'a str,
    // Optionally specify a custom array of form values to scrape and post.
    //
    // Defaults to `["form_build_id", "form_id"]` (Drupal 8+ defaults).
    form_values: &'a [&'a str],
    // Optionally validate the page with the search form.
    //
    // Defaults to doing no validation.
    search_page_validation: Option<&'a crate::Validate<'a>>,
    // Optionally set a custom `op` name for the search button.
    //
    // Defaults to `Search`.
    submit: &'a str,
    // Optionally validate the search results page.
    //
    // Defaults to doing no validation.
    results_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> SearchParams<'a> {
    /// Convenience function to bring [`SearchParamsBuilder`] into scope.
    pub fn builder() -> SearchParamsBuilder<'a> {
        SearchParamsBuilder::new()
    }
}

/// Used to build a [`SearchParams`] object, necessary to invoke the [`search`] function.
///
/// Performing a search on a Drupal 8+ website is generally as simple as follow:
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::drupal;
///
/// transaction!(search);
///
/// async fn search(user: &mut GooseUser) -> TransactionResult {
///     // Define the search parameters.
///     let search_params = drupal::SearchParams::builder()
///         // Search for the keys "search terms".
///         .keys("search terms")
///         .build();
///
///     // Perform the actual search.
///     let _search_results_html = drupal::search(user, &search_params).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/support.rs"); }
/// # let server = support::search_server("search", "Search", &[("keys", "search terms"), ("op", "Search")], "Results");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(search(&mut user));
/// # assert!(result.is_ok(), "doc example SearchParamsBuilder: {}", support::failure(&result));
/// ```
///
/// The builder can also be used to customize the search parameters, for example:
///
/// # Customized Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::{Validate, drupal};
///
/// transaction!(search);
///
/// async fn search(user: &mut GooseUser) -> TransactionResult {
///     // Define the search parameters.
///     // Verify that the search form page has a title that includes "Custom Search".
///     let validate_search_page = Validate::builder()
///         .title("Custom Search")
///         .build();
///     // Verify that the search results page has a title that includes the search terms.
///     let validate_results_page = Validate::builder()
///         .title("search terms")
///         .build();
///     let search_params = drupal::SearchParams::builder()
///         // Search for the keys "search terms".
///         .keys("search terms")
///         // Use a search form on a custom path.
///         .url("custom/search/path")
///         // Perform the search page validation defined above.
///         .search_page_validation(&validate_search_page)
///         // Use a submit button named "Custom Search".
///         .submit("Custom Search")
///         // Perform the search results validation defined above.
///         .results_page_validation(&validate_results_page)
///         .build();
///
///     // Perform the actual search.
///     let _search_results_html = drupal::search(user, &search_params).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/support.rs"); }
/// # let server = support::search_server("custom/search/path", "Custom Search", &[("keys", "search terms"), ("op", "Custom Search")], "search terms");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(search(&mut user));
/// # assert!(result.is_ok(), "doc example SearchParamsBuilder, customized: {}", support::failure(&result));
/// ```
pub struct SearchParamsBuilder<'a> {
    keys: &'a str,
    url: &'a str,
    form_values: &'a [&'a str],
    search_page_validation: Option<&'a crate::Validate<'a>>,
    submit: &'a str,
    results_page_validation: Option<&'a crate::Validate<'a>>,
}
impl<'a> SearchParamsBuilder<'a> {
    // Internally used when building to set defaults.
    fn new() -> Self {
        Self {
            // Defaults to empty search keys.
            keys: "",
            // Defaults to "search".
            url: "search",
            // Defaults to form values required by Drupal 8 and 9.
            form_values: &["form_build_id", "form_id"],
            // Defaults to no extra search page validation.
            search_page_validation: None,
            // Defaults to a search button named "Search".
            submit: "Search",
            // Defaults to no extra results page validation.
            results_page_validation: None,
        }
    }

    /// Used with [`SearchParams::builder`] to set the keys to search for.
    ///
    /// Defaults to `""`, an empty search string.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Use Drupal's default search form to search for "foo bar".
    /// let search_params = SearchParams::builder()
    ///     .keys("foo bar")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("keys", "foo bar")], "Results");
    /// # support::search("keys", &server, &search_params);
    /// ```
    pub fn keys(mut self, keys: impl Into<&'a str>) -> Self {
        self.keys = keys.into();
        self
    }

    /// Used with [`SearchParams::builder`] to set the url the search form is on.
    ///
    /// Defaults to `search`, Drupal's default path for the search form.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Use a search form on custom path `custom/search/path`.
    /// let search_params = SearchParams::builder()
    ///     .url("custom/search/path")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("custom/search/path", "Search", &[], "Results");
    /// # support::search("url", &server, &search_params);
    /// ```
    pub fn url(mut self, url: impl Into<&'a str>) -> Self {
        self.url = url.into();
        self
    }

    /// Used with [`SearchParams::builder`] to set form_values that are extracted from
    /// the search form and used when POSTing the search.
    ///
    /// Defaults to form values required by Drupal 8 and 9: `&["form_build_id", "form_id"]`.
    /// See the example below for how to perform a search on a Drupal 7 website.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Perform a search on a Drupal 7 website.
    /// let search_params = SearchParams::builder()
    ///     // Searching on Drupal 7 also requires `form_token`.
    ///     .form_values(&["form_token", "form_build_id", "form_id"])
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("form_token", support::search_token())], "Results");
    /// # support::search("form_values", &server, &search_params);
    /// ```
    pub fn form_values(mut self, form_values: &'a [&'a str]) -> Self {
        self.form_values = form_values;
        self
    }

    /// Used with [`SearchParams::builder`] to tell the [`search`] function to perform
    /// extra validation of the page containing the search form.
    ///
    /// Defaults to `None`, so no extra validation is performed. By default it will still
    /// validate that the search request returns a valid HTTP response code, and it will
    /// load all static assets on the page with the search form.
    ///
    /// What validation should be performed is defined by passing a reference to a
    /// [`Validate`](../struct.Validate.html) object.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Validate the title of the search page.
    /// let validate_search_page = Validate::builder()
    ///     .title("Custom Search")
    ///     .build();
    /// let search_params = SearchParams::builder()
    ///     .search_page_validation(&validate_search_page)
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Custom Search", &[], "Results");
    /// # support::search("search_page_validation", &server, &search_params);
    /// ```
    pub fn search_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.search_page_validation = Some(validation);
        self
    }

    /// Used with [`SearchParams::builder`] to set a custom search form submit `op`.
    ///
    /// Defaults to Drupal's standard search button name of `Search`.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Perform a search with a custom search button.
    /// let search_params = SearchParams::builder()
    ///     // Searching on Drupal 7 also requires `form_token`.
    ///     .submit("Custom Search")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("op", "Custom Search")], "Results");
    /// # support::search("submit", &server, &search_params);
    /// ```
    pub fn submit(mut self, submit: impl Into<&'a str>) -> Self {
        self.submit = submit.into();
        self
    }

    /// Used with [`SearchParams::builder`] to tell the [`search`] function to validate
    /// the page title containing the search results.
    ///
    /// Defaults to `None`, so no extra validation is performed. By default it will still
    /// validate that the search response returns a valid HTTP response code, and it will
    /// load all static assets on the returned search results page.
    ///
    /// What validation should be performed is defined by passing a reference to a
    /// [`Validate`](../struct.Validate.html) object.
    ///
    /// Once built, the resulting object is passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::Validate;
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Validate that the search terms are in the title of the search results.
    /// let validate_results_page = Validate::builder()
    ///     .title("foo")
    ///     .build();
    /// let search_params = SearchParams::builder()
    ///     .keys("foo")
    ///     .results_page_validation(&validate_results_page)
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("keys", "foo")], "Search for foo");
    /// # support::search("results_page_validation", &server, &search_params);
    /// ```
    pub fn results_page_validation(mut self, validation: &'a crate::Validate) -> Self {
        self.results_page_validation = Some(validation);
        self
    }

    /// Build the [`SearchParams`] object which is then passed to the [`search`] function.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::SearchParams;
    ///
    /// // Use the default search form to search for `example keys`.
    /// let search_params = SearchParams::builder()
    ///     .keys("example keys")
    ///     .build();
    /// # mod support { include!("../tests/doc/support.rs"); }
    /// # let server = support::search_server("search", "Search", &[("keys", "example keys")], "Results");
    /// # support::search("SearchParamsBuilder::build", &server, &search_params);
    /// ```
    pub fn build(self) -> SearchParams<'a> {
        let Self {
            keys,
            url,
            form_values,
            search_page_validation,
            submit,
            results_page_validation,
        } = self;
        SearchParams {
            keys,
            url,
            form_values,
            search_page_validation,
            submit,
            results_page_validation,
        }
    }
}

/// Perform a simple Drupal-powered search.
///
/// In the following example, [`SearchParamsBuilder::keys`] is used to configure the
/// keys being searched for, and [`SearchParamsBuilder::search_page_validation`] is
/// used to validate that the page with the search form has a title containing `Search`.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::drupal;
///
/// transaction!(search);
///
/// async fn search(user: &mut GooseUser) -> TransactionResult {
///     // Use the default search form to search for "foo", validating that the
///     // search page has a title of Search.
///     let validate_search_page = &goose_eggs::Validate::builder()
///         .title("Search")
///         .build();
///     let search_params = drupal::SearchParams::builder()
///         .keys("foo")
///         .search_page_validation(validate_search_page)
///         .build();
///     // Perform the actual search.
///     let _search_results = drupal::search(user, &search_params).await?;
///
///     Ok(())
/// }
/// # mod support { include!("../tests/doc/support.rs"); }
/// # let server = support::search_server("search", "Search", &[("keys", "foo")], "Results");
/// # let mut user = support::user(&server);
/// # let result = support::block_on(search(&mut user));
/// # assert!(result.is_ok(), "doc example search: {}", support::failure(&result));
/// ```
pub async fn search<'a>(
    user: &mut GooseUser,
    params: &'a SearchParams<'a>,
) -> Result<String, Box<TransactionError>> {
    // Load the search page.
    let goose = user.get(params.url).await?;

    // Optionally validate the page with the search form.
    let no_validation = crate::Validate::none();
    let validate = if let Some(validation) = params.search_page_validation {
        validation
    } else {
        &no_validation
    };
    let search_page = crate::validate_and_load_static_assets(user, goose, validate).await?;

    // Extract the search form from the page.
    let search_form = get_form(&search_page, "search-form");

    // Extract values from the search form.
    let form_values = get_form_values(&search_form, params.form_values);

    // Build search form.
    let keys = params.keys.to_string();
    let submit = params.submit.to_string();
    let mut search_params = vec![("keys", keys), ("op", submit)];
    for value in params.form_values {
        search_params.push((*value, form_values.get(value).unwrap().to_string()));
    }

    // Perform the search.
    let goose = user.post_form(params.url, &search_params).await?;

    // Optionally validate the search results page.
    let validate = if let Some(validation) = params.results_page_validation {
        validation
    } else {
        &no_validation
    };
    let search_results = crate::validate_and_load_static_assets(user, goose, validate).await?;

    // Return the search results.
    Ok(search_results)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pages captured from a Drupal 11 Umami site by scripts/capture-fixtures.sh.
    const LOGIN_PAGE: &str = include_str!("../tests/fixtures/drupal/user-login.html");
    const SEARCH_PAGE: &str = include_str!("../tests/fixtures/drupal/search.html");

    // A BigPipe replacement holding two encoded forms, as Drupal embeds them.
    const BIGPIPE: &str = concat!(
        r#"<script type="application/vnd.drupal-ajax">"#,
        r#"[{"command":"insert","data":"<form data-drupal-selector="comment-form">"#,
        r#"<input name="form_build_id" value="form-first" \/>"#,
        r#"<input name="form_id" value="comment_form" \/><\/form>"}]"#,
        "\n</script>\n",
        r#"<script type="application/vnd.drupal-ajax">"#,
        r#"[{"command":"insert","data":"<form data-drupal-selector="other-form">"#,
        r#"<input name="form_build_id" value="form-second" \/><\/form>"}]"#,
        "\n</script>",
    );

    #[test]
    fn get_form_cases() {
        // (case, html, form name, expected form)
        let cases = [
            (
                "by id",
                r#"<form id="a" method="post"><input name="x"></form>"#,
                "a",
                r#"<input name="x">"#,
            ),
            (
                "by data-drupal-selector",
                r#"<form data-drupal-selector="b" id="b--2"><p>B</p></form>"#,
                "b",
                "<p>B</p>",
            ),
            (
                "second of two",
                r#"<form id="a">A</form><form id="b">B</form>"#,
                "b",
                "B",
            ),
            (
                "longer name first",
                r#"<form id="a-extra">X</form><form id="a">Y</form>"#,
                "a",
                "Y",
            ),
            (
                "multiline",
                "<form id=\"a\">\n<p>\nA\n</p>\n</form>",
                "a",
                "<p>A</p>",
            ),
            ("missing", r#"<form id="a">A</form>"#, "b", ""),
        ];
        for (case, html, name, expected) in cases {
            assert_eq!(get_form(html, name), expected, "get_form case {}", case);
        }

        // On a real Drupal page the log in form comes after the search block form.
        let form = get_form(LOGIN_PAGE, "user-login-form");
        assert!(
            form.contains(r#"name="pass""#) && form.contains(r#"value="user_login_form""#),
            "get_form case drupal login page: login form not returned: {}",
            form
        );
        assert!(
            !form.contains("search-block-form") && !form.contains("</form>"),
            "get_form case drupal login page: more than the login form returned: {}",
            form
        );
    }

    #[test]
    fn get_form_value_cases() {
        let form = concat!(
            r#"<input type="hidden" name="form_build_id" value="form-abc" />"#,
            r#"<input type="hidden" name="form_id" value='single_quoted' />"#,
            r#"<input type="text" name="subject[0][value]" value="bracketed" />"#,
            r#"<input type="text" name="empty" value="" />"#,
            r#"<input type="text" name="form_id_extra" value="not this" />"#,
            r#"<input type="text" name="twice" value="first" />"#,
            r#"<input type="text" name="twice" value="second" />"#,
        );
        // (case, element name, expected value)
        let cases = [
            ("double quoted", "form_build_id", "form-abc"),
            ("single quoted", "form_id", "single_quoted"),
            ("regex characters in name", "subject[0][value]", "bracketed"),
            ("empty value", "empty", ""),
            ("first of two", "twice", "first"),
            ("missing", "form_token", "none"),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                get_form_value(form, name),
                expected,
                "get_form_value case {}",
                case
            );
        }

        let search_form = get_form(SEARCH_PAGE, "search-form");
        assert_eq!(
            get_form_value(&search_form, "form_id"),
            "search_form",
            "get_form_value case drupal search page"
        );
    }

    #[test]
    fn get_bigpipe_form_cases() {
        // (case, html, form name, text the form must contain, text it must not)
        let cases = [
            (
                "first form",
                BIGPIPE,
                "comment-form",
                "form-first",
                "form-second",
            ),
            (
                "second form",
                BIGPIPE,
                "other-form",
                "form-second",
                "form-first",
            ),
        ];
        for (case, html, name, contains, excludes) in cases {
            let form = get_bigpipe_form(html, name);
            assert!(
                form.contains(contains) && !form.contains(excludes),
                "get_bigpipe_form case {}: wrong form returned: {}",
                case,
                form
            );
            assert!(
                !form.contains("}]"),
                "get_bigpipe_form case {}: ran past the replacement: {}",
                case,
                form
            );
        }
        assert_eq!(
            get_bigpipe_form(BIGPIPE, "missing-form"),
            "",
            "get_bigpipe_form case missing"
        );
        assert_eq!(
            get_bigpipe_form(LOGIN_PAGE, "user-login-form"),
            "",
            "get_bigpipe_form case plain form is not a bigpipe form"
        );
    }

    #[test]
    fn get_encoded_form_value_cases() {
        let form = get_bigpipe_form(BIGPIPE, "comment-form");
        // (case, element name, expected value)
        let cases = [
            ("form_build_id", "form_build_id", "form-first"),
            ("form_id", "form_id", "comment_form"),
            ("missing", "form_token", "none"),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                get_encoded_form_value(&form, name),
                expected,
                "get_encoded_form_value case {}",
                case
            );
        }
    }

    #[test]
    fn get_updated_build_id_cases() {
        let response = concat!(
            r#"[{"command":"settings"},"#,
            r#"{"command":"update_build_id","old":"form-old","new":"form-new"},"#,
            r#"{"command":"update_build_id","old":"form-other","new":"form-other-new"}]"#,
        );
        // (case, old build id, expected new build id)
        let cases = [
            ("first", "form-old", "form-new"),
            ("second", "form-other", "form-other-new"),
            ("unknown old id", "form-unknown", "none"),
        ];
        for (case, old, expected) in cases {
            assert_eq!(
                get_updated_build_id(response, old),
                expected,
                "get_updated_build_id case {}",
                case
            );
        }
    }

    #[test]
    fn get_form_values_cases() {
        let form = get_form(LOGIN_PAGE, "user-login-form");
        let values = get_form_values(&form, &["form_id", "name", "form_token", "form_build_id"]);
        assert_eq!(values.len(), 4, "get_form_values case count");
        // (case, element name, expected value)
        let cases = [
            ("form_id", "form_id", "user_login_form"),
            ("empty value", "name", ""),
            ("missing", "form_token", "none"),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                values.get(name).map(String::as_str),
                Some(expected),
                "get_form_values case {}",
                case
            );
        }
        assert!(
            values
                .get("form_build_id")
                .is_some_and(|id| id.starts_with("form-")),
            "get_form_values case drupal build id: {:?}",
            values.get("form_build_id")
        );
    }

    #[test]
    fn get_encoded_form_values_cases() {
        let form = get_bigpipe_form(BIGPIPE, "comment-form");
        let values = get_encoded_form_values(&form, &["form_build_id", "form_id", "form_token"]);
        assert_eq!(values.len(), 3, "get_encoded_form_values case count");
        // (case, element name, expected value)
        let cases = [
            ("form_build_id", "form_build_id", "form-first"),
            ("form_id", "form_id", "comment_form"),
            ("missing", "form_token", "none"),
        ];
        for (case, name, expected) in cases {
            assert_eq!(
                values.get(name).map(String::as_str),
                Some(expected),
                "get_encoded_form_values case {}",
                case
            );
        }
    }
}
