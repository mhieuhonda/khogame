use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "postgres://khogame@127.0.0.1:55432/khogame";
    let pool = PgPoolOptions::new()
        .max_connections(15)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(url)
        .await?;
    eprintln!("pool OK, chạy migrate...");
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => eprintln!("MIGRATE OK"),
        Err(e) => eprintln!("MIGRATE FAIL: {e}"),
    }
    Ok(())
}
