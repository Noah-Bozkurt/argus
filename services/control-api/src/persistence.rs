use sqlx::postgres::PgPoolOptions;

pub async fn bootstrap_postgres(database_url: &str) -> Result<(), sqlx::Error> {
    let pool = PgPoolOptions::new().max_connections(5).connect(database_url).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(())
}
