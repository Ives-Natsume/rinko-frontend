use crate::{config::QQConfig, utils::BotAdapter};
use crate::utils::*;
use crate::backend::connection_manager::BackendConnectionManager;
use uuid::Uuid;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use axum::{
    extract::State,
    routing::post,
    Json,
    Router,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};

const QQ_ACCESS_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQ_AUTHORIZE_URL: &str = "https://api.sgroup.qq.com";

#[derive(Deserialize)]
#[allow(dead_code)]
struct WebhookPayload {
    op: u8,
    d: serde_json::Value,
    #[serde(default)]
    t: Option<String>,
    #[serde(default)]
    s: Option<u32>,
}

#[derive(Deserialize)]
struct ValidationRequest {
    plain_token: String,
    event_ts: String,
}

#[derive(Serialize)]
struct ValidationResponse {
    plain_token: String,
    signature: String,
}

#[allow(unused)]
#[derive(Deserialize, Debug)]
struct GroupMessageEvent {
    id: String,
    group_openid: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    timestamp: String,
}

#[derive(Serialize, Debug)]
struct SendGroupMessageRequest {
    content: Option<String>,
    msg_type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    msg_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    msg_seq: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    media: Option<MediaInfo>,
}

#[derive(Serialize, Debug)]
struct MediaInfo {
    file_info: String,
}

#[derive(Serialize, Debug)]
struct UploadMediaRequest {
    file_type: u8,
    url: String,
    srv_send_msg: bool,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct UploadMediaResponse {
    file_uuid: String,
    file_info: String,
    ttl: u32,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct SendMessageResponse {
    pub id: String,
    pub timestamp: i64,
}

#[derive(Clone)]
struct WebhookState {
    client_secret: String,  // used as bot_secret for signature verification
    qq_config: Arc<RwLock<QQConfig>>,
    backend_manager: Option<Arc<BackendConnectionManager>>,
}

/// Webhook handler for QQ bot events
async fn handle_webhook(
    State(state): State<Arc<WebhookState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    tracing::debug!("Received webhook request");
    tracing::debug!("Headers: {:#?}", headers);
    tracing::debug!("Body: {:#?}", body);

    // Parse payload to check op code
    let payload: WebhookPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Failed to parse payload: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response();
        }
    };

    // Handle different operation codes
    match payload.op {
        13 => {
            // op=13: Configuration validation (when setting up webhook URL)
            tracing::info!("Handling webhook configuration validation (op=13)");
            handle_validation(&state.client_secret, payload.d).await
        }
        0 => {
            // op=0: Runtime event dispatch
            tracing::debug!("Handling runtime event (op=0)");
            
            // Verify signature for runtime events
            let sig_header = headers.get("X-Signature-Ed25519")
                .and_then(|v| v.to_str().ok());
            let timestamp_header = headers.get("X-Signature-Timestamp")
                .and_then(|v| v.to_str().ok());

            let (sig_hex, timestamp) = match (sig_header, timestamp_header) {
                (Some(s), Some(t)) => (s, t),
                _ => {
                    tracing::warn!("Missing signature headers");
                    return (StatusCode::UNAUTHORIZED, "Missing signature headers").into_response();
                }
            };

            if let Err(e) = verify_runtime_signature(&state.client_secret, sig_hex, timestamp, &body) {
                tracing::error!("Signature verification failed: {}", e);
                return (StatusCode::UNAUTHORIZED, "Invalid signature").into_response();
            }

            tracing::debug!("Signature verified successfully");
            
            // Handle the event
            if let Some(event_type) = &payload.t {
                tracing::info!("Event type: {}", event_type);
                let event_id = payload.d.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());
                handle_event(&state.qq_config, &state.backend_manager, event_type, &payload.d, event_id).await;
            }

            (StatusCode::NO_CONTENT, "").into_response()
        }
        _ => {
            tracing::warn!("Unknown op code: {}", payload.op);
            (StatusCode::BAD_REQUEST, "Unknown operation").into_response()
        }
    }
}

/// Handle webhook configuration validation (op=13)
async fn handle_validation(
    client_secret: &str,
    data: serde_json::Value,
) -> Response {
    let validation: ValidationRequest = match serde_json::from_value(data) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to parse validation request: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid validation data").into_response();
        }
    };

    // Generate signature for validation
    let signature = match generate_validation_signature(client_secret, &validation.event_ts, &validation.plain_token) {
        Ok(sig) => sig,
        Err(e) => {
            tracing::error!("Failed to generate signature: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate signature").into_response();
        }
    };

    let response = ValidationResponse {
        plain_token: validation.plain_token,
        signature,
    };

    tracing::info!("Validation response generated successfully");
    Json(response).into_response()
}

/// Handle event processing
async fn handle_event(
    qq_config: &Arc<RwLock<QQConfig>>,
    backend_manager: &Option<Arc<BackendConnectionManager>>,
    event_type: &str,
    data: &serde_json::Value,
    event_id: Option<String>,
) {
    match event_type {
        "READY" => tracing::info!("Bot is ready"),
        "GROUP_AT_MESSAGE_CREATE" => {
            handle_group_at_message(qq_config, backend_manager, data, event_id).await;
        }
        "MESSAGE_CREATE" | "C2C_MESSAGE_CREATE" => {
            tracing::debug!("Message received: {:#?}", data);
            // TODO: Process other message types
        }
        "GUILD_CREATE" => tracing::info!("Joined a guild"),
        "FRIEND_ADD" => tracing::info!("Friend added"),
        "GROUP_ADD_ROBOT" => {
            tracing::info!("Added to group: {:#?}", data);
            // Could send a welcome message here
        }
        _ => tracing::debug!("Unhandled event type: {}", event_type),
    }
}

/// Handle GROUP_AT_MESSAGE_CREATE event
async fn handle_group_at_message(
    qq_config: &Arc<RwLock<QQConfig>>,
    backend_manager: &Option<Arc<BackendConnectionManager>>,
    data: &serde_json::Value,
    event_id: Option<String>,
) {
    tracing::debug!("Group @ message received: {:#?}", data);
    
    // Parse message event
    let msg_event = match serde_json::from_value::<GroupMessageEvent>(data.clone()) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("Failed to parse GROUP_AT_MESSAGE_CREATE event: {}", e);
            return;
        }
    };

    tracing::debug!(
        "Parsed message - ID: {}, Group: {}, Content: '{}'",
        msg_event.id,
        msg_event.group_openid,
        msg_event.content
    );

    let content_trimmed = msg_event.content.trim();
    
    // Parse command from message
    // let parsed_cmd = ParsedCommand::parse(content_trimmed);
    
    // Report message to backend if connected
    if let Some(manager) = backend_manager {
        let state = manager.state().await;
        
        if state == crate::backend::connection_manager::ConnectionState::Connected {
            let unified_msg = UnifiedMessage {
                event_id: Uuid::now_v7(),
                content: content_trimmed.to_string(),
                platform: Platform::QQ,
            };
            
            // Create metadata with command info and message context
            let mut metadata = HashMap::new();
            metadata.insert("group_openid".to_string(), msg_event.group_openid.clone());
            metadata.insert("message_id".to_string(), msg_event.id.clone());
            if let Some(ref eid) = event_id {
                metadata.insert("event_id".to_string(), eid.clone());
            }
            
            // Try to report to backend
            let client_lock = manager.client();
            if let Some(client) = &mut *client_lock.write().await {
                match client.report_message(unified_msg, metadata.clone()).await {
                    Ok(response) => {
                        tracing::debug!(
                            "Message reported to backend",
                        );
                        
                        // If backend returns a response message, send it
                        if !response.message.is_empty() && response.message != "OK" {
                            let config = qq_config.read().await;
                            let _ = config.send_group_message(
                                &msg_event.group_openid,
                                &response.message,
                                Some(msg_event.id.clone()),
                                event_id.clone(),
                                Some(1),
                            ).await;
                        }
                        
                        // Backend handled the message, return early
                        return;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to report message to backend: {}. Marking as disconnected.", e);
                        manager.mark_disconnected().await;
                    }
                }
            }
        } else {
            tracing::debug!("Backend offline, using local processing");
        }
    }
    
    // Local fallback processing if backend is disabled or failed
    let config = qq_config.read().await;

    let reply_content = format!("\nRinko backend offline >_\nMessage received: {}", content_trimmed);
    
    if let Err(e) = config.send_group_message(
        &msg_event.group_openid,
        &reply_content,
        Some(msg_event.id),
        event_id,
        Some(1),
    ).await {
        tracing::error!("Failed to send reply: {}", e);
    }
}

/// Generate signature for configuration validation (op=13)
fn generate_validation_signature(
    client_secret: &str,
    event_ts: &str,
    plain_token: &str,
) -> anyhow::Result<String> {
    // Generate signing key from client_secret
    let mut seed = client_secret.to_string();
    while seed.len() < 32 {
        seed.push_str(client_secret);
    }
    let seed_bytes: [u8; 32] = seed.as_bytes()[0..32]
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to create seed"))?;

    let signing_key = SigningKey::from_bytes(&seed_bytes);

    // Construct message: event_ts + plain_token
    let mut message = Vec::new();
    message.extend_from_slice(event_ts.as_bytes());
    message.extend_from_slice(plain_token.as_bytes());

    // Sign and encode as hex
    let signature = signing_key.sign(&message);
    Ok(hex::encode(signature.to_bytes()))
}

/// Verify Ed25519 signature for runtime events (op=0)
fn verify_runtime_signature(
    client_secret: &str,  // client_secret is used as bot_secret
    sig_hex: &str,
    timestamp: &str,
    body: &str,
) -> anyhow::Result<()> {
    // 1. Generate public key from client_secret (used as bot_secret)
    // According to docs: seed = secret + secret (until >= 32 bytes)
    let mut seed = client_secret.to_string();
    while seed.len() < 32 {
        seed.push_str(client_secret);
    }
    let seed_bytes: [u8; 32] = seed.as_bytes()[0..32]
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to create seed"))?;

    // Generate keypair to extract public key
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed_bytes);
    let verifying_key: VerifyingKey = (&signing_key).into();

    // 2. Decode signature from hex
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| anyhow::anyhow!("Failed to decode signature hex: {}", e))?;
    let signature = Signature::from_slice(&sig_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid signature format: {}", e))?;

    // 3. Construct message: timestamp + body
    let mut message = Vec::new();
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body.as_bytes());

    // 4. Verify signature
    verifying_key
        .verify(&message, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))?;

    Ok(())
}

#[async_trait]
impl BotAdapter for QQConfig {
    async fn process_message(&self) -> anyhow::Result<UnifiedMessage> {
        // Implementation for processing a message from QQ
        let message = UnifiedMessage {
            event_id: Uuid::now_v7(),
            content: "Sample QQ message".to_string(),
            platform: Platform::QQ,
        };
        Ok(message)
    }

    async fn send_message(&self, message: &UnifiedMessage) -> anyhow::Result<()> {
        // Implementation for sending a message via QQ
        println!("Sending message to QQ: {:#?}", message);
        Ok(())
    }
}

impl QQConfig {
    pub async fn init(&mut self) -> anyhow::Result<()> {
        self.client = reqwest::Client::new();
        self.get_access_token().await
    }

    /// Start webhook server to receive QQ bot events
    pub async fn start_webhook_server(
        qq_config: Arc<RwLock<Self>>,
        backend_manager: Option<Arc<BackendConnectionManager>>,
        port: u16,
    ) -> anyhow::Result<()> {
        let client_secret = qq_config.read().await.client_secret.clone();
        
        let state = Arc::new(WebhookState {
            client_secret,
            qq_config: qq_config.clone(),
            backend_manager,
        });

        let app = Router::new()
            .route("/webhook", post(handle_webhook))
            .with_state(state);

        let addr = format!("127.0.0.1:{}", port);
        tracing::info!("Starting QQ webhook server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    pub async fn get_access_token(&mut self) -> anyhow::Result<()> {
        let resp = self.client
            .post(QQ_ACCESS_TOKEN_URL)
            .json(&serde_json::json!({
                "appId": self.app_id,
                "clientSecret": self.client_secret
            }))
            .send()
            .await?
            .error_for_status()?;

        let resp_json: serde_json::Value = resp.json().await?;
        
        let token = resp_json
            .get("access_token")
            .and_then(|v| v.as_str());

        let expire = resp_json
            .get("expires_in")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok());

        if let (Some(token), Some(expire)) = (token, expire) {
            tracing::info!("Obtained QQ access token: {}, expires in: {}", token, expire);
            self.access_token = token.to_string();
            self.token_expires_in = expire;
            self.token_fetched_at = Some(tokio::time::Instant::now());
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to get access token from QQ: {:?}", resp_json))
        }
    }

    /// Starts a background task that automatically renews the access token before it expires
    pub fn start_token_renewal_task(config: Arc<RwLock<Self>>) {
        tokio::spawn(async move {
            loop {
                let refresh_delay = {
                    let cfg = config.read().await;
                    if let Some(fetched_at) = cfg.token_fetched_at {
                        let elapsed = fetched_at.elapsed().as_secs();
                        let lifetime = cfg.token_expires_in;
                        // refresh 50 seconds before expiry
                        let refresh_after = lifetime.saturating_sub(50);
                        if elapsed >= refresh_after {
                            Duration::from_secs(0)
                        } else {
                            Duration::from_secs(refresh_after - elapsed)
                        }
                    } else {
                        Duration::from_secs(0)
                    }
                };
                
                tracing::debug!("QQ token renewal scheduled in {:?}", refresh_delay);
                sleep(refresh_delay).await;
                
                tracing::info!("Attempting to renew QQ access token...");
                let mut cfg = config.write().await;
                if let Err(e) = cfg.get_access_token().await {
                    tracing::error!("Failed to renew QQ access token: {}. Retrying in 30 seconds.", e);
                    drop(cfg); // Release lock before sleeping
                    sleep(Duration::from_secs(30)).await;
                } else {
                    tracing::info!("QQ access token renewed successfully.");
                }
            }
        });
    }

    /// Upload media file and get file_info
    /// 
    /// # Parameters
    /// - `group_openid`: The openid of the target group
    /// - `file_type`: 1=image, 2=video, 3=voice, 4=file
    /// - `url`: URL of the media resource (must be accessible by QQ servers)
    /// - `srv_send_msg`: Whether to send message directly (not recommended)
    async fn upload_group_media(
        &self,
        group_openid: &str,
        file_type: u8,
        url: &str,
        srv_send_msg: bool,
    ) -> anyhow::Result<UploadMediaResponse> {
        let api_url = format!("{}/v2/groups/{}/files", QQ_AUTHORIZE_URL, group_openid);
        
        let payload = UploadMediaRequest {
            file_type,
            url: url.to_string(),
            srv_send_msg,
        };

        tracing::debug!("Uploading media to group {}: {:?}", group_openid, payload);

        let resp = self.client
            .post(&api_url)
            .header("Authorization", format!("QQBot {}", self.access_token))
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?;

        let response: UploadMediaResponse = resp.json().await?;
        tracing::info!(
            "Media uploaded - UUID: {}, TTL: {}s",
            response.file_uuid,
            response.ttl
        );

        Ok(response)
    }

    /// Send rich media message to group
    /// 
    /// # Parameters
    /// - `group_openid`: The openid of the target group
    /// - `file_info`: The file_info obtained from upload_group_media
    /// - `msg_id`: Optional message ID for passive reply
    /// - `event_id`: Optional event ID for passive message
    /// - `msg_seq`: Optional message sequence number
    async fn send_group_media_message(
        &self,
        group_openid: &str,
        file_info: &str,
        msg_id: Option<String>,
        event_id: Option<String>,
        msg_seq: Option<u32>,
    ) -> anyhow::Result<SendMessageResponse> {
        let url = format!("{}/v2/groups/{}/messages", QQ_AUTHORIZE_URL, group_openid);
        
        let payload = SendGroupMessageRequest {
            content: None,
            msg_type: 7, // 7 = rich media
            msg_id,
            event_id,
            msg_seq,
            media: Some(MediaInfo {
                file_info: file_info.to_string(),
            }),
        };

        tracing::debug!("Sending media message to group {}: {:?}", group_openid, payload);

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("QQBot {}", self.access_token))
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;

        let response: SendMessageResponse = resp.json().await?;
        tracing::info!(
            "Media message sent successfully - ID: {}, Timestamp: {}",
            response.id,
            response.timestamp
        );

        Ok(response)
    }

    /// Send image from local file to group
    /// This is a high-level function that handles the complete workflow
    /// 
    /// # Parameters
    /// - `group_openid`: The openid of the target group
    /// - `local_path`: Path to the local image file
    /// - `msg_id`: Optional message ID for passive reply
    /// - `event_id`: Optional event ID for passive message
    /// - `msg_seq`: Optional message sequence number
    pub async fn send_group_image(
        &self,
        group_openid: &str,
        local_path: &str,
        msg_id: Option<String>,
        event_id: Option<String>,
        msg_seq: Option<u32>,
    ) -> anyhow::Result<SendMessageResponse> {
        // For local files, we need to provide a URL accessible by QQ servers
        // In production, you would upload to a CDN or temporary hosting
        // For now, we'll use a public URL as a placeholder
        
        // TODO: Implement actual file upload or local HTTP server
        // For testing, you need to replace this with an actual accessible URL
        let image_url = "https://metasequoiani.com/_astro/image.BnwkcnDf_Z2vIi4L.webp"; // Replace with actual URL
        
        tracing::warn!(
            "Local file path '{}' needs to be accessible via URL. Using placeholder: {}",
            local_path,
            image_url
        );
        
        // Step 1: Upload media and get file_info
        let upload_response = self.upload_group_media(
            group_openid,
            1, // 1 = image
            image_url,
            false, // Don't send directly, get file_info for flexible usage
        ).await?;

        // Step 2: Send media message using file_info
        self.send_group_media_message(
            group_openid,
            &upload_response.file_info,
            msg_id,
            event_id,
            msg_seq,
        ).await
    }

    /// Send message to a group chat
    /// 
    /// # Parameters
    /// - `group_openid`: The openid of the target group
    /// - `content`: Message content
    /// - `msg_id`: Optional message ID for passive reply (within 5 minutes)
    /// - `event_id`: Optional event ID for passive message
    /// - `msg_seq`: Optional message sequence number (default: 1)
    pub async fn send_group_message(
        &self,
        group_openid: &str,
        content: &str,
        msg_id: Option<String>,
        event_id: Option<String>,
        msg_seq: Option<u32>,
    ) -> anyhow::Result<SendMessageResponse> {
        let url = format!("{}/v2/groups/{}/messages", QQ_AUTHORIZE_URL, group_openid);
        
        let payload = SendGroupMessageRequest {
            content: Some(content.to_string()),
            msg_type: 0, // 0 = text message
            msg_id,
            event_id,
            msg_seq,
            media: None,
        };

        tracing::debug!("Sending message to group {}: {:#?}", group_openid, payload);

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("QQBot {}", self.access_token))
            .json(&payload)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .error_for_status()?;

        let response: SendMessageResponse = match resp.json().await {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to parse send message response: {}", e);
                return Err(anyhow::anyhow!("Failed to parse send message response: {}", e));
            }
        };
        tracing::info!(
            "Message sent successfully - ID: {}, Timestamp: {}",
            response.id,
            response.timestamp
        );

        Ok(response)
    }
}