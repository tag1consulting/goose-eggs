//! The `wordpress` helpers against a real WordPress site: the theme test data
//! site from examples/wordpress-sites/theme-test-data, which the `wordpress` CI
//! job starts. Set `WORDPRESS_SITE` to its URL to run it; without it the test
//! does nothing, so a plain `cargo test` stays offline.
//!
//! It needs a live site, so it is not in tests/sabotage/LEDGER.tsv, whose breaks
//! every run offline; like the Umami example, its proof is its CI job.
//!
//!   examples/wordpress-sites/theme-test-data/setup.sh start
//!   WORDPRESS_SITE=http://localhost:18081 cargo test --test wordpress_site -- --nocapture
#[path = "../common/mod.rs"]
mod common;

use common::{check_named, run, transaction, Expect};
use goose::prelude::*;
use goose_eggs::wordpress::{self, Comment, Discover, Login, Post, SearchParams, SiteContent};
use std::env;
use std::sync::{Mutex, OnceLock};

const DISCOVER: &str = "discover finds posts, pages, categories and tags";
const LOG_IN: &str = "log_in as the editor";
const SEARCH: &str = "search for a word in the test data";
const COMMENT: &str = "post_comment as the editor";
const EDIT: &str = "create_post then update_post through REST";
const LOG_OUT: &str = "log_out from the dashboard";

static CONTENT: OnceLock<SiteContent> = OnceLock::new();
static EDITED: Mutex<Option<String>> = Mutex::new(None);

fn editor() -> (String, String) {
    (
        env::var("EDITOR_USERNAME").unwrap_or_else(|_| "editor".to_string()),
        env::var("EDITOR_PASSWORD").unwrap_or_else(|_| "editor-password".to_string()),
    )
}

#[tokio::test]
async fn wordpress_site() {
    let site = match env::var("WORDPRESS_SITE") {
        Ok(site) => site,
        Err(_) => {
            eprintln!("wordpress_site: WORDPRESS_SITE is not set, nothing to test");
            return;
        }
    };

    let metrics = run(
        &site,
        vec![
            transaction(DISCOVER, |user| {
                Box::pin(async move {
                    let content = wordpress::discover(user, &Discover::builder().build()).await?;
                    let _ = CONTENT.set(content);
                    Ok(())
                })
            }),
            transaction(LOG_IN, |user| {
                Box::pin(async move {
                    let (username, password) = editor();
                    let login = Login::builder()
                        .username(username.as_str())
                        .password(password.as_str())
                        .build();
                    wordpress::log_in(user, &login).await?;
                    Ok(())
                })
            }),
            transaction(SEARCH, |user| {
                Box::pin(async move {
                    let params = SearchParams::builder().keys("post").build();
                    let html = wordpress::search(user, &params).await?;
                    assert!(html.contains("search-results"), "no search results page");
                    Ok(())
                })
            }),
            transaction(COMMENT, |user| {
                Box::pin(async move {
                    // WordPress's own first post, which takes comments.
                    let comment = Comment::builder().post_url("hello-world/").build();
                    wordpress::post_comment(user, &comment).await?;
                    Ok(())
                })
            }),
            transaction(EDIT, |user| {
                Box::pin(async move {
                    let html = user.get("wp-admin/").await?.response?.text().await?;
                    let nonce = wordpress::get_rest_nonce(&html).unwrap_or_default();
                    let draft = Post::builder().title("Goose draft").build();
                    let created = wordpress::create_post(user, &nonce, &draft).await?;
                    let id = created["id"].as_u64().unwrap_or_default();
                    let title = format!("Goose edit {}", goose_eggs::text::random_word(8));
                    let edit = Post::builder().title(&title).build();
                    let updated = wordpress::update_post(user, &nonce, id, &edit).await?;
                    *EDITED.lock().unwrap() = updated["title"]["raw"].as_str().map(str::to_string);
                    if updated["title"]["raw"].as_str() != Some(title.as_str()) {
                        return Err(Box::new(TransactionError::Custom(format!(
                            "title not updated: {}",
                            updated["title"]
                        ))));
                    }
                    Ok(())
                })
            }),
            transaction(LOG_OUT, |user| {
                Box::pin(async move {
                    wordpress::log_out(user, "wp-admin/").await?;
                    Ok(())
                })
            }),
        ],
    )
    .await;

    let mut problems: Vec<String> = vec![
        check_named(
            &metrics,
            DISCOVER,
            &["wp-sitemap-posts-post-1.xml"],
            Expect::Pass,
        ),
        check_named(&metrics, LOG_IN, &["wp-login.php"], Expect::Pass),
        check_named(&metrics, SEARCH, &["search"], Expect::Pass),
        check_named(
            &metrics,
            COMMENT,
            &["comment form page", "wp-comments-post.php"],
            Expect::Pass,
        ),
        check_named(
            &metrics,
            EDIT,
            &["wp-json/wp/v2/posts", "wp-json/wp/v2/posts/{id}"],
            Expect::Pass,
        ),
        check_named(
            &metrics,
            LOG_OUT,
            &["wp-admin/", "wp-login.php?action=logout"],
            Expect::Pass,
        ),
    ]
    .into_iter()
    .flatten()
    .collect();

    let content = CONTENT.get().cloned().unwrap_or_default();
    for kind in ["post", "page", "category", "post_tag"] {
        if content.urls(kind).is_empty() {
            problems.push(format!("behaviour {}: no {} found", DISCOVER, kind));
        }
    }
    let errors: Vec<String> = metrics.errors.values().map(|e| e.error.clone()).collect();
    if !errors.is_empty() {
        problems.push(format!("request errors in the run: {:?}", errors));
    }
    println!(
        "wordpress_site: {} posts, {} pages, {} categories, {} tags discovered; edited title {:?}; {} requests",
        content.posts().len(),
        content.pages().len(),
        content.categories().len(),
        content.tags().len(),
        EDITED.lock().unwrap(),
        metrics
            .requests
            .values()
            .map(|r| r.success_count + r.fail_count)
            .sum::<usize>()
    );
    common::assert_all(problems);
}
