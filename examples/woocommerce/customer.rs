/// The username and password the Goose user at `index` (its
/// `weighted_users_index`) logs in with.
///
/// With a `username`, every user logs in to that one account. Otherwise each
/// user has its own account, `customer1` for the first user, `customer2` for
/// the second and so on, as the demo shop's setup.sh creates them: WordPress
/// keeps one cart and one list of login sessions per account, so users sharing
/// an account empty each other's carts and drop each other's logins. The
/// password defaults to the demo shop's `customer-password`.
pub fn credentials(
    index: usize,
    username: Option<String>,
    password: Option<String>,
) -> (String, String) {
    (
        username.unwrap_or_else(|| format!("customer{}", index + 1)),
        password.unwrap_or_else(|| "customer-password".to_string()),
    )
}
