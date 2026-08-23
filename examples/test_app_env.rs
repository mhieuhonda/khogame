#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match dotenvy::from_filename(".env") {
        Ok(p) => eprintln!("parsed OK from {:?}", p),
        Err(e) => eprintln!("parse ERR: {}", e),
    }
    eprintln!("DATABASE_URL = {:?}", std::env::var("DATABASE_URL").unwrap_or_default());
    Ok(())
}
