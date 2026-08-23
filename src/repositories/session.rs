use crate::error::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

pub struct SessionRepo;

impl SessionRepo {
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        token_hash: &str,
        user_agent: &str,
        ip: Option<&str>,
        ttl_days: i64,
    ) -> AppResult<Uuid> {
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO sessions (user_id, token_hash, user_agent, ip_address, expires_at)
              VALUES ($1, $2, $3, $4, NOW() + ($5 || ' days')::INTERVAL)
              RETURNING id"#,
        )
        .bind(user_id)
        .bind(token_hash)
        .bind(user_agent)
        .bind(ip)
        .bind(ttl_days.to_string())
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn find_user_by_token(
        pool: &PgPool,
        token_hash: &str,
    ) -> AppResult<Option<Uuid>> {
        let user_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT user_id FROM sessions
              WHERE token_hash = $1 AND expires_at > NOW()"#,
        )
        .bind(token_hash)
        .fetch_optional(pool)
        .await?;
        Ok(user_id)
    }

    pub async fn delete(pool: &PgPool, token_hash: &str) -> AppResult<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete_all_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired(pool: &PgPool) -> AppResult<()> {
        sqlx::query("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(pool)
            .await?;
        Ok(())
    }
}
