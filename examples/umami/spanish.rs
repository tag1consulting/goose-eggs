use goose::prelude::*;

use crate::common;

use rand::seq::SliceRandom;

/// Load the front page in Spanish and all static assets found on the page.
pub async fn front_page_es(user: &GooseUser) -> GooseTaskResult {
    let goose = user.get("/es").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title("Inicio"),
    )
    .await?;

    Ok(())
}

/// Load article listing in Spanish and all static assets found on the page.
pub async fn recipe_listing_es(user: &GooseUser) -> GooseTaskResult {
    let goose = user.get("/es/recipes/").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title("Recetas"),
    )
    .await?;

    Ok(())
}

/// Load a random recipe in Spanish and all static assets found on the page.
pub async fn recipe_es(user: &GooseUser) -> GooseTaskResult {
    let nodes = common::get_nodes(&common::ContentType::Recipe);
    let recipe = nodes.choose(&mut rand::thread_rng());
    let goose = user.get(recipe.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title(recipe.unwrap().title_es),
    )
    .await?;

    Ok(())
}

/// Load article listing in Spanish and all static assets found on the page.
pub async fn article_listing_es(user: &GooseUser) -> GooseTaskResult {
    let goose = user.get("/es/articles/").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title("Artículos"),
    )
    .await?;

    Ok(())
}

/// Load a random article in Spanish and all static assets found on the page.
pub async fn article_es(user: &GooseUser) -> GooseTaskResult {
    let nodes = common::get_nodes(&common::ContentType::Article);
    let article = nodes.choose(&mut rand::thread_rng());
    let goose = user.get(article.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title(article.unwrap().title_es),
    )
    .await?;

    Ok(())
}

/// Load a basic page in Spanish and all static assets found on the page.
pub async fn basic_page_es(user: &GooseUser) -> GooseTaskResult {
    let nodes = common::get_nodes(&common::ContentType::BasicPage);
    let page = nodes.choose(&mut rand::thread_rng());
    let goose = user.get(page.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title(page.unwrap().title_es),
    )
    .await?;

    Ok(())
}

/// Anonymously load the contact form in Spanish and POST feedback.
pub async fn anonymous_contact_form_es(user: &GooseUser) -> GooseTaskResult {
    common::anonymous_contact_form(user, false).await?;

    Ok(())
}

// Pick a random word from the title of a random node and perform a search in Spanish.
pub async fn search_es(user: &GooseUser) -> GooseTaskResult {
    // Build a random three-word phrase taken from Spanish words on the Umami website.
    let search_words = common::random_words(3, false);
    let search_phrase = search_words.join(" ");

    let search_params = goose_eggs::drupal::SearchParams::keys(&search_phrase)
        .update_url("/es/search/node")
        .update_title("Buscar");
    goose_eggs::drupal::search(user, &search_params).await?;

    Ok(())
}

/// Load category listing by a random term in Spanish and all static assets found on the page.
pub async fn term_listing_es(user: &GooseUser) -> GooseTaskResult {
    let terms = common::get_terms();
    let term = terms.choose(&mut rand::thread_rng());
    let goose = user.get(term.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::title(term.unwrap().title_es),
    )
    .await?;

    Ok(())
}
