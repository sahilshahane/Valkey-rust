use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;


#[derive(FromRow, Debug, Serialize, Deserialize)]
pub struct KVValue {
    pub value: String
}

#[derive(Deserialize)]
pub struct SetValueRequest {
    pub value: String
}