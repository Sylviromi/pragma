use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::str::FromStr;

pub async fn init_db(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(db_path)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(opts).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(include_str!("migrations/001_init.sql"))
        .execute(pool)
        .await?;
    sqlx::query(include_str!("migrations/002_active_loops.sql"))
        .execute(pool)
        .await?;
    sqlx::query(include_str!("migrations/003_file_snapshots.sql"))
        .execute(pool)
        .await?;
    // 004 adds a column (SQLite can't ALTER IF NOT EXISTS) — guard manually.
    let has_heartbeat: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('active_loops') WHERE name='heartbeat_at'",
    )
    .fetch_one(pool)
    .await?;
    if has_heartbeat == 0 {
        sqlx::query(include_str!("migrations/004_active_loops_heartbeat.sql"))
            .execute(pool)
            .await?;
    }
    sqlx::query(include_str!("migrations/005_telemetry.sql"))
        .execute(pool)
        .await?;
    // 006 adds columns (SQLite can't ALTER IF NOT EXISTS) — guard manually.
    let has_sys: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('session_usage') WHERE name='system_tokens'",
    )
    .fetch_one(pool)
    .await?;
    if has_sys == 0 {
        sqlx::query(include_str!("migrations/006_usage_breakdown.sql"))
            .execute(pool)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_create_sessions_table() {
        let pool = init_db(":memory:").await.unwrap();
        let (name,): (String,) =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' AND name='sessions'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "sessions");
    }

    #[tokio::test]
    async fn test_migrations_idempotent() {
        let pool = init_db(":memory:").await.unwrap();
        run_migrations(&pool).await.unwrap(); // second run should not error
    }
}
