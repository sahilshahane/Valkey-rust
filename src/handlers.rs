use axum::{
    Json, extract::{Path, State}, http::{StatusCode}, response::{IntoResponse, Response}
};
use crate::{AppState, Arc, models::{SetValueRequest}};


pub async fn get_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
) -> Response {

    if let Some(value) = state.cache.get(&key) {
        // tracing::debug!("Cache HIT for key: {}", key);
        return (StatusCode::OK, value.value().clone()).into_response();
    }

    // tracing::debug!("Cache MISS for key: {}", key);

    // let result = sqlx::query_as::<_, KVValue>("SELECT value FROM kv_store WHERE key = $1")
    //     .bind(&key)
    //     .fetch_optional(&(*state.pool))
    //     .await.unwrap();


    // if let Some(kv) = result {
    //     state.cache.insert(key, kv.value.clone()).await;
    //     return (StatusCode::OK, kv.value).into_response()
    // }

    return (StatusCode::NOT_FOUND).into_response();
}


pub async fn set_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>,
    Json(payload): Json<SetValueRequest>,
) -> Response {



    // sqlx::query("INSERT INTO kv_store (key, value) VALUES ($1, $2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
    //     .bind(&key)
    //     .bind(&payload.value)
    //     .execute(&(*state.pool))
    //     .await.unwrap();

    if let Err(error) = state.wal.set(&key, &payload.value){
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
    }

    state.cache.insert(key, payload.value);

    return (StatusCode::OK).into_response();
}

pub async fn delete_key(
    State(state): State<Arc<AppState>>,
    Path(key): Path<String>
) -> Response {
    
    // let result = sqlx::query("DELETE FROM kv_store where key = $1")
    //     .bind(&key)
    //     .execute(&(*state.pool))
    //     .await.unwrap();

    // if result.rows_affected() == 0 {
    //     return (StatusCode::NOT_FOUND).into_response();
    // }

    if !state.cache.contains_key(&key) {
        return (StatusCode::NOT_FOUND).into_response();
    }

    if let Err(error) = state.wal.delete(&key){
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
    }

    state.cache.remove(&key);

    return (StatusCode::OK).into_response();
}