//! The WooCommerce example's choice of account for each returning customer.
//! Users sharing one account empty each other's carts (checkout answers
//! `409 Conflict`) and drop each other's logins, so each user needs its own.
#[path = "../examples/woocommerce/customer.rs"]
mod customer;

use customer::credentials;
use std::collections::HashSet;

#[test]
fn credentials_cases() {
    // (case, user index, CUSTOMER_USERNAME, CUSTOMER_PASSWORD, expected)
    let cases = [
        (
            "first user",
            0,
            None,
            None,
            ("customer1", "customer-password"),
        ),
        (
            "eighth user",
            7,
            None,
            None,
            ("customer8", "customer-password"),
        ),
        ("password", 7, None, Some("secret"), ("customer8", "secret")),
        (
            "one account",
            7,
            Some("shopper"),
            None,
            ("shopper", "customer-password"),
        ),
        (
            "one account with password",
            97,
            Some("shopper"),
            Some("secret"),
            ("shopper", "secret"),
        ),
    ];
    for (case, index, username, password, (expected_username, expected_password)) in cases {
        let (got_username, got_password) = credentials(
            index,
            username.map(str::to_string),
            password.map(str::to_string),
        );
        assert_eq!(
            (got_username.as_str(), got_password.as_str()),
            (expected_username, expected_password),
            "credentials case {case}: wrong account for user {index}"
        );
    }

    // At 100 users the returning customers are users 7, 17 and so on to 97;
    // every user, returning customer or not, gets an account of its own.
    let accounts: HashSet<String> = (0..100)
        .map(|index| credentials(index, None, None).0)
        .collect();
    assert_eq!(
        accounts.len(),
        100,
        "credentials case per user: 100 users share {} accounts",
        accounts.len()
    );
}
