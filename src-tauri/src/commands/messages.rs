use crate::AppState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String, // JSON: string or ContentBlock[]
    pub model: String,
    pub provider: String,
    pub timestamp: String,
    pub tokens: i64,
}

#[derive(Debug, Deserialize)]
pub struct NewMessage {
    pub session_id: String,
    pub role: String,
    pub content: String, // serialised JSON
    pub model: String,
    pub provider: String,
    pub tokens: i64,
}

#[tauri::command]
pub async fn add_message(msg: NewMessage, state: tauri::State<'_, AppState>) -> Result<Message, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,?,?,?,?,?,?)")
        .bind(&id).bind(&msg.session_id).bind(&msg.role)
        .bind(&msg.content).bind(&msg.model).bind(&msg.provider)
        .bind(&now).bind(msg.tokens)
        .execute(&state.db).await.map_err(|e| e.to_string())?;
    sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id=?")
        .bind(&id).fetch_one(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_messages(session_id: String, state: tauri::State<'_, AppState>) -> Result<Vec<Message>, String> {
    sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE session_id=? ORDER BY timestamp ASC")
        .bind(&session_id).fetch_all(&state.db).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;

    #[tokio::test]
    async fn test_add_and_list_messages() {
        let pool = init_db(":memory:").await.unwrap();
        let sid = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO sessions (id,name,working_dir,created_at,updated_at,model,provider,archived,metadata_json) VALUES (?,?,'',?,?,'','',0,'{}')")
            .bind(&sid).bind("s").bind(&now).bind(&now).execute(&pool).await.unwrap();
        let mid = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO messages (id,session_id,role,content,model,provider,timestamp,tokens) VALUES (?,?,'user','\"hello\"','','',?,0)")
            .bind(&mid).bind(&sid).bind(&now).execute(&pool).await.unwrap();
        let msgs = sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE session_id=?")
            .bind(&sid).fetch_all(&pool).await.unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "\"hello\"");
    }
}