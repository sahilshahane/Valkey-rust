use axum::{
    Json, extract::{Path, State}, http::{Error, StatusCode}, response::{IntoResponse, Response}
};
use crate::{AppState, Arc, error::Result, models::{KVValue, SetValueRequest}};


pub async fn get_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Response {
    let result: std::result::Result<Option<KVValue>, sqlx::Error> = sqlx::query_as::<_, KVValue>("SELECT value FROM kv_store WHERE key = ?")
        .bind(&key)
        .fetch_optional(&state.pool)
        .await;

    if let Err(err) = result {
        return (StatusCode::INTERNAL_SERVER_ERROR).into_response();
    }

    let val = result.unwrap();

    return match val {
        Some(val) => (StatusCode::OK, val.value).into_response(),
        None => (StatusCode::NOT_FOUND).into_response(),
    } 
}


pub async fn set_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(payload): Json<SetValueRequest>,
) -> Response {
    
    sqlx::query("INSERT INTO kv_store (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(&key)
        .bind(&payload.value)
        .execute(&state.pool)
        .await.unwrap();

    return (StatusCode::OK).into_response();
}

pub async fn delete_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>
) -> Response {
    
    let result = sqlx::query("DELETE FROM kv_store where key = ?")
        .bind(&key)
        .execute(&state.pool)
        .await.unwrap();

    if result.rows_affected() == 0 {
        return (StatusCode::NOT_FOUND).into_response();
    }

    (StatusCode::OK).into_response()
}