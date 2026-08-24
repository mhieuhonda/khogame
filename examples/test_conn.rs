// Test kết nối sqlx 0.9 tới postgres local
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "postgres://khogame@127.0.0.1:55432/khogame";
    eprintln!("1) PgPoolOptions.connect...");
    match PgPoolOptions::new().connect(url).await {
        Ok(pool) => {
            let v: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(&pool)
                .await?;
            eprintln!("OK: {v}");
        }
        Err(e) => eprintln!("FAIL: {e:?}"),
    }
    eprintln!("2) PgConnection::connect...");
    use sqlx::Connection;
    match sqlx::PgConnection::connect(url).await {
        Ok(mut c) => {
            let v: String = sqlx::query_scalar("SELECT version()")
                .fetch_one(&mut c)
                .await?;
            eprintln!("OK: {v}");
        }
        Err(e) => eprintln!("FAIL: {e:?}"),
    }
    Ok(())
}
