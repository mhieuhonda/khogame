use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "postgres://khogame@127.0.0.1:55432/khogame";
    eprintln!("Với min_connections(2) + timeouts như db.rs...");
    let pool = PgPoolOptions::new()
        .max_connections(15)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(Some(std::time::Duration::from_secs(300)))
        .max_lifetime(Some(std::time::Duration::from_secs(1800)))
        .connect(url)
        .await;
    match pool {
        Ok(p) => {
            let v: String = sqlx::query_scalar("SELECT version()").fetch_one(&p).await?;
            eprintln!("OK: {}", v);
        }
        Err(e) => eprintln!("FAIL: {:?}", e),
    }
    Ok(())
}
