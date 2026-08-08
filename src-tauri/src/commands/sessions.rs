use crate::AppState;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub name: String,
    pub working_dir: String,
    pub created_at: String,
    pub updated_at: String,
    pub model: String,
    pub provider: String,
    pub archived: bool,
    pub metadata_json: String,
}

#[tauri::command]
pub async fn create_session(name: String, state: tauri::State<'_, AppState>) -> Result<Session, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,?,?,?,'','',0,'{}')")
        .bind(&id).bind(&name).bind("").bind(&now).bind(&now)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    get_by_id(&state.db, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<Session>, String> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0 ORDER BY updated_at DESC")
        .fetch_all(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(id: String, state: tauri::State<'_, AppState>) -> Result<Session, String> {
    get_by_id(&state.db, &id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_session(
    id: String, model: String, provider: String, working_dir: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE sessions SET model=?,provider=?,working_dir=?,updated_at=? WHERE id=?")
        .bind(&model).bind(&provider).bind(&working_dir).bind(&now).bind(&id)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn archive_session(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET archived=1 WHERE id=?")
        .bind(&id).execute(&state.db).await.map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Session, sqlx::Error> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id=?")
        .bind(id).fetch_one(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    #[tokio::test]
    async fn test_create_and_list() {
        let pool = init_db(":memory:").await.unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&id).bind("test").bind(&now).bind(&now)
            .execute(&pool).await.unwrap();
        let rows = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "test");
    }

    #[tokio::test]
    async fn test_archive_hides_session() {
        let pool = init_db(":memory:").await.unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&id).bind("s").bind(&now).bind(&now)
            .execute(&pool).await.unwrap();
        sqlx::query("UPDATE sessions SET archived=1 WHERE id=?").bind(&id).execute(&pool).await.unwrap();
        let rows = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE archived=0")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(rows.len(), 0);
    }
}