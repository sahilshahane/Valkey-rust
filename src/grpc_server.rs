use std::sync::Arc;

use tonic::{transport::Server, Request, Response, Status};

use kvstore_grpc::k_vstore_server::{KVstore,KVstoreServer};
use kvstore_grpc::{GetKeyReply,KeyRequest, SetKeyRequest, Void};

use crate::AppState;

pub mod kvstore_grpc {
    tonic::include_proto!("kvstore");
}

pub struct KVStoreGRPC {
    state:  Arc<AppState>
}


#[tonic::async_trait]
impl KVstore for KVStoreGRPC {
    async fn get_key(
        &self,
        request: Request<KeyRequest>,
    ) -> Result<Response<GetKeyReply>, Status> {
        let key = request.into_inner().key;

        if let Some(value) = self.state.cache.get(&key) {
            // tracing::debug!("Cache HIT for key: {}", key);
            return Ok(Response::new(kvstore_grpc::GetKeyReply {
                value: value.value().clone(),
            }));
        }

        // Return empty value for missing keys (treat as successful)
        Ok(Response::new(kvstore_grpc::GetKeyReply {
            value: String::new(),
        }))
    }

    async fn set_key(
        &self,
        request: Request<SetKeyRequest>,
    ) -> Result<Response<Void>, Status> {
        // println!("[SET] Received request from: {:?}", request);

        let payload = request.into_inner();
        let key = payload.key;
        let value = payload.value;
            
        if let Err(error) = self.state.wal.set(&key, &value) {
            return Err(Status::internal(format!("Failed to write to WAL: {}", error)));
        }

        self.state.cache.insert(key, value);

        Ok(Response::new(kvstore_grpc::Void{}))
    }

    async fn delete_key(
        &self,
        request: Request<KeyRequest>,
    ) -> Result<Response<Void>, Status> {
        // println!("[DELETE] Received request from: {:?}", request);

        let key = request.into_inner().key;

        // Check if key exists
        if !self.state.cache.contains_key(&key) {
            return Err(Status::not_found(""));
        }

        // Write to WAL
        if let Err(error) = self.state.wal.delete(&key) {
            return Err(Status::internal(format!("Failed to write to WAL: {}", error)));
        }

        // Remove from cache
        self.state.cache.remove(&key);

        Ok(Response::new(kvstore_grpc::Void{}))
    }
}


pub async fn run_grpc_server(state: Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:4000".parse()?;
    let kvstore = KVStoreGRPC{
        state
    };

    tracing::info!("Starting gRPC Server on 127.0.0.1:4000...");
    Server::builder()
        .add_service(KVstoreServer::new(kvstore))
        .serve(addr)
        .await?;

    Ok(())
}