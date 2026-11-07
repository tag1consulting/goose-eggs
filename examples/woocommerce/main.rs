mod common;
mod customer;
mod load_test;

use goose::prelude::*;

/// Runs the load test `load_test::load_test` defines.
#[tokio::main]
async fn main() -> Result<(), GooseError> {
    let _goose_metrics = load_test::load_test(GooseAttack::initialize()?)?
        .execute()
        .await?;

    Ok(())
}
