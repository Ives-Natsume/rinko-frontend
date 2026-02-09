use tonic::transport::Channel;
use tonic::Request;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::backend::proto::{
    bot_backend_client::BotBackendClient,
    UnifiedMessage as ProtoUnifiedMessage,
    MessageResponse,
    BotCommand,
    SubscribeRequest,
    HeartbeatRequest,
    HeartbeatResponse,
    Platform as ProtoPlatform,
};
use crate::utils::{UnifiedMessage, Platform};

/// gRPC client wrapper for communicating with the backend
pub struct BackendClient {
    client: BotBackendClient<Channel>,
    frontend_id: String,
}

impl BackendClient {
    /// Create a new backend client
    pub async fn new(backend_url: &str, frontend_id: String) -> Result<Self> {
        let channel = Channel::from_shared(backend_url.to_string())?
            .connect()
            .await?;
        
        let client = BotBackendClient::new(channel);
        
        tracing::info!("Connected to backend at {}", backend_url);
        
        Ok(Self {
            client,
            frontend_id,
        })
    }

    /// Report a message to the backend
    pub async fn report_message(&mut self, msg: UnifiedMessage, metadata: HashMap<String, String>) -> Result<MessageResponse> {
        let proto_msg = Self::to_proto_message(msg, metadata);
        
        let request = Request::new(proto_msg);
        let response = self.client.report_message(request).await?;
        
        Ok(response.into_inner())
    }

    /// Subscribe to commands from backend (server streaming)
    pub async fn subscribe_commands(&mut self, platforms: Vec<Platform>) -> Result<tonic::Streaming<BotCommand>> {
        let proto_platforms: Vec<i32> = platforms
            .into_iter()
            .map(|p| Self::to_proto_platform(p) as i32)
            .collect();

        let request = Request::new(SubscribeRequest {
            frontend_id: self.frontend_id.clone(),
            platforms: proto_platforms,
        });

        let response = self.client.subscribe_commands(request).await?;
        
        Ok(response.into_inner())
    }

    /// Send heartbeat to backend
    pub async fn heartbeat(&mut self, status: HashMap<String, String>) -> Result<HeartbeatResponse> {
        let request = Request::new(HeartbeatRequest {
            frontend_id: self.frontend_id.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            status,
        });

        let response = self.client.heartbeat(request).await?;
        
        Ok(response.into_inner())
    }

    /// Start bidirectional chat stream
    pub async fn bidirectional_chat(
        &mut self,
    ) -> Result<(
        tokio::sync::mpsc::Sender<UnifiedMessage>,
        tonic::Streaming<BotCommand>,
    )> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<UnifiedMessage>(100);
        
        // Convert Rust messages to proto messages
        let outbound = async_stream::stream! {
            while let Some(msg) = rx.recv().await {
                yield Self::to_proto_message(msg, HashMap::new());
            }
        };

        let response = self.client.bidirectional_chat(Request::new(outbound)).await?;
        
        Ok((tx, response.into_inner()))
    }

    // Conversion helpers
    fn to_proto_message(msg: UnifiedMessage, metadata: HashMap<String, String>) -> ProtoUnifiedMessage {
        ProtoUnifiedMessage {
            event_id: msg.event_id.to_string(),
            content: msg.content,
            platform: Self::to_proto_platform(msg.platform) as i32,
            timestamp: chrono::Utc::now().timestamp(),
            metadata,
        }
    }

    fn to_proto_platform(platform: Platform) -> ProtoPlatform {
        match platform {
            Platform::QQ => ProtoPlatform::Qq,
            Platform::EnterpriseWeChat => ProtoPlatform::EnterpriseWechat,
            Platform::Telegram => ProtoPlatform::Telegram,
            Platform::Discord => ProtoPlatform::Discord,
        }
    }

    #[allow(dead_code)]
    fn from_proto_platform(platform: i32) -> Platform {
        match ProtoPlatform::try_from(platform) {
            Ok(ProtoPlatform::Qq) => Platform::QQ,
            Ok(ProtoPlatform::EnterpriseWechat) => Platform::EnterpriseWeChat,
            Ok(ProtoPlatform::Telegram) => Platform::Telegram,
            Ok(ProtoPlatform::Discord) => Platform::Discord,
            _ => Platform::QQ, // Default fallback
        }
    }
}

/// Shared backend client that can be used across the application
pub type SharedBackendClient = Arc<RwLock<BackendClient>>;

/// Create a shared backend client
pub async fn create_shared_client(backend_url: &str, frontend_id: String) -> Result<SharedBackendClient> {
    let client = BackendClient::new(backend_url, frontend_id).await?;
    Ok(Arc::new(RwLock::new(client)))
}
