pub mod client;
pub mod connection_manager;

// Include the generated proto code
pub mod proto {
    #![allow(clippy::all)]
    tonic::include_proto!("bot");
}

pub use proto::*;
pub use connection_manager::BackendConnectionManager;
