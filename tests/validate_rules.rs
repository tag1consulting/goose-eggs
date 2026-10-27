//! Every `Validate` rule, passing and failing, in one Goose run against one mock
//! server. A failing rule must become a transaction failure and a request error
//! naming the rule.
mod common;

use common::{check, run, transaction, Expect};
use goose_eggs::{validate_page, Validate, ValidateBuilder};
use httpmock::{Method::GET, MockServer};

const PAGE: &str = r#"<!DOCTYPE html>
<html>
<head>
  <title>Title 1234ABCD</title>
</head>
<body>
  <p>Test text on the page.</p>
  <p>Second paragraph</p>
</body>
</html>
"#;

type Rules = fn(ValidateBuilder<'static>) -> ValidateBuilder<'static>;

struct Case {
    // The behaviour, also the transaction and request name.
    name: &'static str,
    path: &'static str,
    rules: Rules,
    expect: Expect,
}

const CASES: &[Case] = &[
    Case {
        name: "status passes",
        path: "/page",
        rules: |v| v.status(200),
        expect: Expect::Pass,
    },
    Case {
        name: "status fails",
        path: "/page",
        rules: |v| v.status(201),
        expect: Expect::Fail("response status != 201"),
    },
    Case {
        name: "not_status passes",
        path: "/page",
        rules: |v| v.not_status(404),
        expect: Expect::Pass,
    },
    Case {
        name: "not_status fails",
        path: "/page",
        rules: |v| v.not_status(200),
        expect: Expect::Fail("response status == 200"),
    },
    Case {
        name: "title passes in any case",
        path: "/page",
        rules: |v| v.title("title 1234abcd"),
        expect: Expect::Pass,
    },
    Case {
        name: "title fails",
        path: "/page",
        rules: |v| v.title("Missing"),
        expect: Expect::Fail("title not found: Missing"),
    },
    Case {
        name: "not_title passes",
        path: "/page",
        rules: |v| v.not_title("Missing"),
        expect: Expect::Pass,
    },
    Case {
        name: "not_title fails",
        path: "/page",
        rules: |v| v.not_title("1234ABCD"),
        expect: Expect::Fail("title found: 1234ABCD"),
    },
    Case {
        name: "text passes",
        path: "/page",
        rules: |v| v.text("Test text"),
        expect: Expect::Pass,
    },
    Case {
        name: "text fails",
        path: "/page",
        rules: |v| v.text("absent text"),
        expect: Expect::Fail("text not found on page: absent text"),
    },
    Case {
        name: "text is case sensitive",
        path: "/page",
        rules: |v| v.text("test text"),
        expect: Expect::Fail("text not found on page: test text"),
    },
    Case {
        name: "not_text passes",
        path: "/page",
        rules: |v| v.not_text("absent text"),
        expect: Expect::Pass,
    },
    Case {
        name: "not_text fails",
        path: "/page",
        rules: |v| v.not_text("Test text"),
        expect: Expect::Fail("text found on page: Test text"),
    },
    Case {
        name: "texts passes",
        path: "/page",
        rules: |v| v.texts(vec!["Test text", "Second paragraph"]),
        expect: Expect::Pass,
    },
    Case {
        name: "texts fails on its last text",
        path: "/page",
        rules: |v| v.texts(vec!["Test text", "absent second"]),
        expect: Expect::Fail("text not found on page: absent second"),
    },
    Case {
        name: "not_texts passes",
        path: "/page",
        rules: |v| v.not_texts(vec!["absent one", "absent two"]),
        expect: Expect::Pass,
    },
    Case {
        name: "not_texts fails on its last text",
        path: "/page",
        rules: |v| v.not_texts(vec!["absent one", "Second paragraph"]),
        expect: Expect::Fail("text found on page: Second paragraph"),
    },
    Case {
        name: "header passes",
        path: "/page",
        rules: |v| v.header("x-cache"),
        expect: Expect::Pass,
    },
    Case {
        name: "header fails",
        path: "/page",
        rules: |v| v.header("x-missing"),
        expect: Expect::Fail(r#"header not included in response: "x-missing""#),
    },
    Case {
        name: "not_header passes",
        path: "/page",
        rules: |v| v.not_header("x-missing"),
        expect: Expect::Pass,
    },
    Case {
        name: "not_header fails",
        path: "/page",
        rules: |v| v.not_header("x-cache"),
        expect: Expect::Fail(r#"header included in response: "x-cache""#),
    },
    Case {
        name: "header_value passes",
        path: "/page",
        rules: |v| v.header_value("foo", "bar"),
        expect: Expect::Pass,
    },
    Case {
        name: "header_value fails on the value",
        path: "/page",
        rules: |v| v.header_value("foo", "baz"),
        expect: Expect::Fail(r#"header does not contain expected value: "baz""#),
    },
    Case {
        name: "header_value fails on a missing header",
        path: "/page",
        rules: |v| v.header_value("x-missing", "bar"),
        expect: Expect::Fail(r#"header not included in response: "x-missing""#),
    },
    Case {
        name: "not_header_value passes on another value",
        path: "/page",
        rules: |v| v.not_header_value("foo", "baz"),
        expect: Expect::Pass,
    },
    Case {
        name: "not_header_value passes on a missing header",
        path: "/page",
        rules: |v| v.not_header_value("x-missing", "bar"),
        expect: Expect::Pass,
    },
    Case {
        name: "not_header_value fails",
        path: "/page",
        rules: |v| v.not_header_value("foo", "bar"),
        expect: Expect::Fail(r#"header contains unexpected value: "bar""#),
    },
    Case {
        name: "redirect passes",
        path: "/redirect",
        rules: |v| v.redirect(true),
        expect: Expect::Pass,
    },
    Case {
        name: "redirect fails",
        path: "/page",
        rules: |v| v.redirect(true),
        expect: Expect::Fail("did not redirect"),
    },
    Case {
        name: "no redirect passes",
        path: "/page",
        rules: |v| v.redirect(false),
        expect: Expect::Pass,
    },
    Case {
        name: "no redirect fails",
        path: "/redirect",
        rules: |v| v.redirect(false),
        expect: Expect::Fail("redirected unexpectedly"),
    },
    Case {
        name: "every rule passes together",
        path: "/page",
        rules: |v| {
            v.status(200)
                .title("1234ABCD")
                .text("Test text")
                .not_text("absent")
                .header("x-cache")
                .header_value("foo", "bar")
                .not_header("x-missing")
                .redirect(false)
        },
        expect: Expect::Pass,
    },
    Case {
        name: "one failing rule among passing rules fails",
        path: "/page",
        rules: |v| {
            v.status(200)
                .title("1234ABCD")
                .header("x-cache")
                .text("absent among many")
        },
        expect: Expect::Fail("text not found on page: absent among many"),
    },
    Case {
        name: "the last title rule wins",
        path: "/page",
        rules: |v| v.not_title("1234ABCD").title("1234ABCD"),
        expect: Expect::Pass,
    },
];

#[tokio::test]
async fn validate_rules() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/page");
        then.status(200)
            .header("foo", "bar")
            .header("x-cache", "HIT")
            .body(PAGE);
    });
    server.mock(|when, then| {
        when.method(GET).path("/redirect");
        then.status(301).header("location", "/page");
    });

    let mut transactions: Vec<_> = CASES
        .iter()
        .map(|case| {
            transaction(case.name, move |user| {
                Box::pin(async move {
                    let goose = user.get_named(case.path, case.name).await?;
                    let validate = (case.rules)(Validate::builder()).build();
                    validate_page(user, goose, &validate).await?;
                    Ok(())
                })
            })
        })
        .collect();

    // Validate::none() validates nothing and returns the body.
    transactions.push(transaction("none returns the body", |user| {
        Box::pin(async move {
            let goose = user.get_named("/page", "none returns the body").await?;
            let html = validate_page(user, goose, &Validate::none()).await?;
            if html.contains("Second paragraph") {
                Ok(())
            } else {
                Err(format!("body not returned: {:?}", html).into())
            }
        })
    }));

    let metrics = run(&server.base_url(), transactions).await;

    let mut problems: Vec<String> = CASES
        .iter()
        .filter_map(|case| check(&metrics, case.name, case.expect))
        .collect();
    problems.extend(check(&metrics, "none returns the body", Expect::Pass));
    common::assert_all(problems);
}
