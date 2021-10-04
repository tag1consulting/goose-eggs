use goose::prelude::*;

use crate::common;

use rand::seq::SliceRandom;
use std::env;

/// Log into the website.
pub async fn log_in(user: &mut GooseUser) -> GooseTaskResult {
    // Use ADMIN_USERNAME= to set custom admin username.
    let admin_username = match env::var("ADMIN_USERNAME") {
        Ok(username) => username,
        Err(_) => "admin".to_string(),
    };
    // Use ADMIN_PASSWORD= to set custom admin username.
    let admin_password = match env::var("ADMIN_PASSWORD") {
        Ok(password) => password,
        Err(_) => "P@ssw0rd1234".to_string(),
    };

    let login = goose_eggs::drupal::Login::username_password(&admin_username, &admin_password)
        .unwrap()
        .update_url("en/user/login");
    goose_eggs::drupal::log_in(user, login.as_ref()).await?;

    Ok(())
}

/// Load and edit a random article.
pub async fn edit_article(user: &mut GooseUser) -> GooseTaskResult {
    // First, load a random article.
    let nodes = common::get_nodes(&common::ContentType::Article);
    let article = nodes.choose(&mut rand::thread_rng());
    let goose = user.get(article.unwrap().url_en).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title_text(
            article.unwrap().title_en,
            &format!("en/node/{}/edit", article.unwrap().nid),
        ),
    )
    .await?;

    // Next, load the edit link for the chosen article.
    let goose = user
        .get(&format!("en/node/{}/edit", article.unwrap().nid))
        .await?;

    let edit_page = goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title("Edit Article"),
    )
    .await?;

    let edit_form = goose_eggs::drupal::get_form(&edit_page, "node-article-edit-form");
    let form_build_id = goose_eggs::drupal::get_form_value(&edit_form, "form_build_id");
    let form_token = goose_eggs::drupal::get_form_value(&edit_form, "form_token");
    let form_id = goose_eggs::drupal::get_form_value(&edit_form, "form_id");

    // Build node form with random word from title.
    let params = [
        ("form_build_id", &form_build_id),
        ("form_token", &form_token),
        ("form_id", &form_id),
        ("op", &"Save (this translation)".to_string()),
    ];
    let request_builder = user.goose_post(&format!("en/node/{}/edit", article.unwrap().nid))?;
    let mut saved_article = user.goose_send(request_builder.form(&params), None).await?;

    // A successful node save is redirected.
    if !saved_article.request.redirected {
        return user.set_failure(
            &format!("{}: saving article failed", saved_article.request.final_url),
            &mut saved_article.request,
            None,
            None,
        );
    }

    // Be sure we're viewing the same article after editing it.
    goose_eggs::validate_and_load_static_assets(
        user,
        saved_article,
        &goose_eggs::Validate::title(article.unwrap().title_en),
    )
    .await?;

    Ok(())
}
