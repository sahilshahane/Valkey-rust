use sqlx::prelude::FromRow;
use sonic_rs::{Deserialize, Serialize}; 


#[derive(FromRow, Debug, Serialize, Deserialize)]
pub struct KVValue {
    pub value: String
}

#[derive(Deserialize)]
pub struct SetValueRequest {
    pub value: String
}