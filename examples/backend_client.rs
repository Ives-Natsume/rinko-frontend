use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use rinko_frontend::backend::proto::{
    bot_backend_server::{BotBackend, BotBackendServer},
    BotCommand, HeartbeatRequest, HeartbeatResponse, MessageResponse, Platform, SubscribeRequest,
    UnifiedMessage,
};

#[derive(Default)]
struct MockBackend;

#[tonic::async_trait]
impl BotBackend for MockBackend {
    async fn report_message(
        &self,
        request: Request<UnifiedMessage>,
    ) -> Result<Response<MessageResponse>, Status> {
        let msg = request.into_inner();
        let platform = Platform::try_from(msg.platform).unwrap_or(Platform::Unspecified);

        tracing::info!(
            "ReportMessage: platform={:?}, content={}",
            platform,
            msg.content
        );

        let reply = if platform == Platform::Qq {
            format!("\nSay hi to Rinko ^ ^)/\nMessage received:\n「{}」", msg.content)
        } else {
            "OK".to_string()
        };

        let response = MessageResponse {
            success: true,
            message: reply,
            message_id: Uuid::now_v7().to_string(),
        };

        Ok(Response::new(response))
    }

    type SubscribeCommandsStream = ReceiverStream<Result<BotCommand, Status>>;

    async fn subscribe_commands(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeCommandsStream>, Status> {
        let req = request.into_inner();
        tracing::info!(
            "SubscribeCommands: frontend_id={}, platforms={:?}",
            req.frontend_id,
            req.platforms
        );

        let (tx, rx) = mpsc::channel(4);

        let mut parameters = HashMap::new();
        parameters.insert("note".to_string(), "example command".to_string());

        let _ = tx
            .send(Ok(BotCommand {
                command_id: Uuid::now_v7().to_string(),
                command_type: "get_status".to_string(),
                parameters,
                timestamp: Utc::now().timestamp(),
            }))
            .await;

        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(300)).await;
                if tx.is_closed() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            "Heartbeat: frontend_id={}, status_keys={}",
            req.frontend_id,
            req.status.len()
        );

        Ok(Response::new(HeartbeatResponse {
            healthy: true,
            message: "OK".to_string(),
        }))
    }

    type BidirectionalChatStream = ReceiverStream<Result<BotCommand, Status>>;

    async fn bidirectional_chat(
        &self,
        request: Request<tonic::Streaming<UnifiedMessage>>,
    ) -> Result<Response<Self::BidirectionalChatStream>, Status> {
        let mut inbound = request.into_inner();

        tokio::spawn(async move {
            while let Ok(Some(msg)) = inbound.message().await {
                let platform = Platform::try_from(msg.platform).unwrap_or(Platform::Unspecified);
                tracing::info!(
                    "BidirectionalChat: platform={:?}, content={}",
                    platform,
                    msg.content
                );
            }
        });

        let (_tx, rx) = mpsc::channel(4);
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let addr = std::env::var("BACKEND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let addr = addr.parse()?;

    tracing::info!("Mock backend listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(BotBackendServer::new(MockBackend::default()))
        .serve(addr)
        .await?;

    Ok(())
}
