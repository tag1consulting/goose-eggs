use goose::prelude::*;

use crate::common;

use rand::prelude::IndexedRandom;

/// Load the front page in Spanish and all static assets found on the page.
pub async fn front_page_es(user: &mut GooseUser) -> TransactionResult {
    let goose = user.get("es").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder().title("Inicio").build(),
    )
    .await?;

    Ok(())
}

/// Load article listing in Spanish and all static assets found on the page.
pub async fn recipe_listing_es(user: &mut GooseUser) -> TransactionResult {
    let goose = user.get("es/recipes/").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder().title("Recetas").build(),
    )
    .await?;

    Ok(())
}

/// Load a random recipe in Spanish and all static assets found on the page.
pub async fn recipe_es(user: &mut GooseUser) -> TransactionResult {
    let nodes = common::get_nodes(&common::ContentType::Recipe);
    let recipe = nodes.choose(&mut rand::rng());
    let goose = user.get(recipe.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder()
            .title(recipe.unwrap().title_es)
            .build(),
    )
    .await?;

    Ok(())
}

/// Load article listing in Spanish and all static assets found on the page.
pub async fn article_listing_es(user: &mut GooseUser) -> TransactionResult {
    let goose = user.get("es/articles/").await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder().title("Artículos").build(),
    )
    .await?;

    Ok(())
}

/// Load a random article in Spanish and all static assets found on the page.
pub async fn article_es(user: &mut GooseUser) -> TransactionResult {
    let nodes = common::get_nodes(&common::ContentType::Article);
    let article = nodes.choose(&mut rand::rng());
    let goose = user.get(article.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder()
            .title(article.unwrap().title_es)
            .build(),
    )
    .await?;

    Ok(())
}

/// Load a basic page in Spanish and all static assets found on the page.
pub async fn basic_page_es(user: &mut GooseUser) -> TransactionResult {
    let nodes = common::get_nodes(&common::ContentType::BasicPage);
    let page = nodes.choose(&mut rand::rng());
    let goose = user.get(page.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder()
            .title(page.unwrap().title_es)
            .build(),
    )
    .await?;

    Ok(())
}

/// Anonymously load the contact form in Spanish and POST feedback.
pub async fn anonymous_contact_form_es(user: &mut GooseUser) -> TransactionResult {
    common::anonymous_contact_form(user, false).await?;

    Ok(())
}

// Pick a random word from the title of a random node and perform a search in Spanish.
pub async fn search_es(user: &mut GooseUser) -> TransactionResult {
    // Build a random three-word phrase taken from Spanish words on the Umami website.
    let search_words = common::random_words(3, false);
    let search_phrase = search_words.join(" ");

    // The search page should have "Buscar" in the title.
    let validate_search_page = &goose_eggs::Validate::builder().title("Buscar").build();
    // The results page should have the search_phrase in the title.
    let validate_results_page = &goose_eggs::Validate::builder()
        .title(&*search_phrase)
        .build();
    let search_params = goose_eggs::drupal::SearchParams::builder()
        .keys(&*search_phrase)
        .url("es/search/node")
        .search_page_validation(validate_search_page)
        .results_page_validation(validate_results_page)
        .build();
    goose_eggs::drupal::search(user, &search_params).await?;

    Ok(())
}

/// Load category listing by a random term in Spanish and all static assets found on the page.
pub async fn term_listing_es(user: &mut GooseUser) -> TransactionResult {
    let terms = common::get_terms();
    let term = terms.choose(&mut rand::rng());
    let goose = user.get(term.unwrap().url_es).await?;
    goose_eggs::validate_and_load_static_assets(
        user,
        goose,
        &goose_eggs::Validate::builder()
            .title(term.unwrap().title_es)
            .build(),
    )
    .await?;

    Ok(())
}
