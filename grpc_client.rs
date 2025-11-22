use kvstore_grpc::k_vstore_client::{KVstoreClient};
use kvstore_grpc::{GetKeyReply,KeyRequest, SetKeyRequest, Void};

pub mod kvstore_grpc {
    tonic::include_proto!("kvstore");
}

