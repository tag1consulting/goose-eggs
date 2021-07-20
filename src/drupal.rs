//! Functionality that's specific to Drupal.

use goose::prelude::*;
use regex::Regex;
use std::env;

/// Use a regular expression to get the specific form identified by data-drupal-selector.
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
/// assert!(!form.is_none());
/// ```
pub fn get_form(html: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"<form.*data-drupal-selector="{}".*>(.*?)</form>"#,
        name
    ))
    .unwrap();
    // Strip carriage returns to simplify regex.
    let line = html.replace("\n", "");
    // Return the entire form, a subset of the received html.
    re.captures(&line).map(|value| value[0].to_string())
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
/// let form_build_id = get_form_value(&form.unwrap(), "form_build_id");
/// assert_eq!(&form_build_id.unwrap(), "form-bHZME2HeTuevNWQR5Y4pyP8jcAu2dfbHERwoscwnajM");
/// ```
pub fn get_form_value(form_html: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"name="{}" value=['"](.*?)['"]"#, name)).unwrap();
    // Return a specific form value.
    re.captures(&form_html).map(|value| value[1].to_string())
}

/// Set one or more defaults when logging in through the standard drupal user-login-form.
///
/// This object can be passed to [`log_in`] to set a custom default username and/or
/// password and/or log in url.
///
/// # Example
/// ```rust
/// use goose_eggs::drupal::Login;
///
/// fn examples() {
///     // Manually build a Login strucuture with custom username and password.
///     let _login = Login::new(
///         Some("foo"), Some("bar"), None,
///     );
///
///     // Call `Login::username_password` to build the same.
///     let _login = Login::username_password("foo", "bar");
///
///     // Also set a custom log-in url.
///     let _login = Login::username_password_url("foo", "bar", "/custom/login/path");
/// }
pub struct Login<'a> {
    // Optionally set a default username.
    username: Option<&'a str>,
    // Optionally set a default password.
    password: Option<&'a str>,
    // Optionally set a custom default path (otherwise defaults to `/user/login`).
    url: Option<&'a str>,
}
impl<'a> Login<'a> {
    /// Create a new Login object, specifying `username`, `password`, and `url`.
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
    /// );
    /// ```
    pub fn new(
        username: Option<&'a str>,
        password: Option<&'a str>,
        url: Option<&'a str>,
    ) -> Option<Login<'a>> {
        Some(Login {
            username,
            password,
            url,
        })
    }

    /// Create a Login object with a default username of "foo".
    ///
    /// The password will remain the default of "password". The login url will remain the
    /// default of "/user/login". The defaults can still be overridden by the USER and PASS
    /// environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username("foo");
    /// ```
    pub fn username(username: &'a str) -> Option<Login<'a>> {
        Login::new(Some(username), None, None)
    }

    /// Create a Login object with a default password of "bar".
    ///
    /// The username will remain the default of "username". The login url will remain the
    /// default of "/user/login". The defaults can still be overridden by the USER and PASS
    /// environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password("bar");
    /// ```
    pub fn password(password: &'a str) -> Option<Login<'a>> {
        Login::new(None, Some(password), None)
    }

    /// Create a Login object with a default username of "foo" and a default password of
    /// "bar".
    ///
    /// The login url will remain the default of "/user/login".  The defaults can still be
    /// overridden by the USER and PASS environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username_password("foo", "bar");
    /// ```
    pub fn username_password(username: &'a str, password: &'a str) -> Option<Login<'a>> {
        Login::new(Some(username), Some(password), None)
    }

    /// Create a Login object with a default url of "/custom/user/login".
    ///
    /// The username will remain the default of "username", the password will remain the
    /// default of "password". The defaults can still be overridden by the USER and PASS
    /// environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::url("/custom/user/login");
    /// ```
    pub fn url(url: &'a str) -> Option<Login<'a>> {
        Login::new(None, None, Some(url))
    }

    /// Create a Login object with a default username of "foo" and a default url of
    /// "/custom/user/login".
    ///
    /// The password will remain the default of "password". The defaults can still be
    /// overridden by the USER and PASS environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username_url("foo", "/custom/user/login");
    /// ```
    pub fn username_url(username: &'a str, url: &'a str) -> Option<Login<'a>> {
        Login::new(Some(username), None, Some(url))
    }

    /// Create a Login object with a default password of "bar" and a default url of
    /// "/custom/user/login".
    ///
    /// The username will remain the default of "username". The defaults can still
    /// be overridden by the USER and PASS environment variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::password_url("bar", "/custom/user/login");
    /// ```
    pub fn password_url(password: &'a str, url: &'a str) -> Option<Login<'a>> {
        Login::new(None, Some(password), Some(url))
    }

    /// Create a Login object with a default username of "foo", a default password
    /// of "bar" and a default url of "/custom/user/login".
    ///
    /// The defaults can still be overridden by the USER and PASS environment
    /// variables.
    ///
    /// # Example
    /// ```rust
    /// use goose_eggs::drupal::Login;
    ///
    /// let _login = Login::username_password_url("foo", "bar", "/custom/user/login");
    /// ```
    pub fn username_password_url(
        username: &'a str,
        password: &'a str,
        url: &'a str,
    ) -> Option<Login<'a>> {
        Login::new(Some(username), Some(password), Some(url))
    }
}

/// Log into a Drupal website.
///
/// The reference to a GooseUser object is from a Goose task function. The optional
/// pointer to a [`Login`] object can be created to override the username, password, or
/// login url.
///
/// If no default username is set in the [`Login`] object, the function will default to
/// a username of `username` which can be overridden by the `USER` environment variable.
/// If no default password is set in the [`Login`] object, the function will default to
/// a password of `password` which can be overridden by the `PASS` environment variable.
/// If no default url is set in the [`Login`] object, the function will default to a url
/// of `/user/login`.
///
/// # Example
/// ```rust
/// use goose::prelude::*;
/// use goose_eggs::drupal::{log_in, Login};
///
/// task!(login).set_on_start();
///
/// async fn login(user: &GooseUser) -> GooseTaskResult {
///     let _html = log_in(&user, Login::username_password("foo", "bar").as_ref()).await?;
///
///     Ok(())
/// }
///
/// ```
pub async fn log_in(user: &GooseUser, login: Option<&Login<'_>>) -> Result<String, GooseTaskError> {
    // Use the `USER` environment variable if it's set, otherwise use the custom username
    // passed in when calling this function, otherwise use `username`.
    let default_password = "username";
    let username = env::var("USER").unwrap_or_else(|_| match login {
        Some(l) => l.username.unwrap_or(default_password).to_string(),
        None => default_password.to_string(),
    });
    // Use the `PASS` environment variable if it's set, otherwise use the custom password
    // passed in when calling this function, otherwise use `password`.
    let default_password = "password";
    let password = env::var("PASS").unwrap_or_else(|_| match login {
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
        &crate::Validate::text(r#"<form class="user-login-form""#),
    )
    .await?;

    // A web page can have multiple forms, so first get the correct form.
    let login_form = match get_form(&login_page, "user-login-form") {
        Some(form) => form,
        None => {
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
    };

    // Now extract the form_build_id in order to POST to the log in form.
    let form_build_id = match get_form_value(&login_form, "form_build_id") {
        Some(build_id) => build_id,
        None => {
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
    };

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
                "{}: login failed (check USER and PASS)",
                logged_in_user.request.final_url
            ),
            &mut logged_in_user.request,
            headers.as_ref(),
            Some(&html),
        )?;
        // Return an empty string as log-in failed. Enable the debug log to
        // determine why.
        return Ok(html);
    }

    // Check the title to verify that the user is actually logged in.
    let logged_in_page = crate::validate_and_load_static_assets(
        user,
        logged_in_user,
        &crate::Validate::title(&username),
    )
    .await?;

    Ok(logged_in_page)
}
