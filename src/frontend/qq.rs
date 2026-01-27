use crate::{config::QQConfig, utils::BotAdapter};
use crate::utils::*;
use uuid::Uuid;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};
use ed25519_dalek::{Signer, SigningKey};
use axum::{
    extract::State,
    routing::post,
    Json,
    Router,
};

const QQ_ACCESS_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQ_AUTHORIZE_URL: &str = "https://api.sgroup.qq.com";

#[derive(Deserialize)]
struct Payload {
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct ValidationRequest {
    event_ts: String,
    plain_token: String,
}

#[derive(Serialize)]
struct ValidationResponse {
    plain_token: String,
    signature: String,
}

struct AppState {
    bot_secret: String,
}

async fn handle_validation(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Payload>,
) -> anyhow::Result<Json<ValidationResponse>> {
    // 1. Parse the inner "data" field
    let validation: ValidationRequest = serde_json::from_value(payload.data)
        .map_err(|e| anyhow::anyhow!("Parse data err: {}", e))?;

    // 2. Derive the seed (32 bytes) from bot_secret
    let mut seed = state.bot_secret.clone();
    while seed.len() < 32 {
        seed.push_str(&state.bot_secret);
    }
    let seed_bytes = &seed.as_bytes()[0..32];
    
    // 3. Generate SigningKey
    let secret_key: [u8; 32] = seed_bytes.try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert seed to 32-byte array"))?;
    let signing_key = SigningKey::from_bytes(&secret_key);

    // 4. Create message: event_ts + plain_token
    let mut message = Vec::new();
    message.extend_from_slice(validation.event_ts.as_bytes());
    message.extend_from_slice(validation.plain_token.as_bytes());

    // 5. Sign and hex encode
    let signature = signing_key.sign(&message);
    let signature_hex = hex::encode(signature.to_bytes());

    Ok(Json(ValidationResponse {
        plain_token: validation.plain_token,
        signature: signature_hex,
    }))
}

#[async_trait]
impl BotAdapter for QQConfig {
    async fn process_message(&self) -> anyhow::Result<UnifiedMessage> {
        // Implementation for processing a message from QQ
        let message = UnifiedMessage {
            enevt_id: Uuid::now_v7(),
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

    pub async fn test_send_message(&self) -> anyhow::Result<()> {
        let http_url = format!("{}/v2/groups/{}/messages", QQ_AUTHORIZE_URL, "850923669");
        let json_payload = serde_json::json!({
            "content": "Shirokane Rinko desu >_",
            "msg_type": 0,
        });
        let resp = self.client
            .post(&http_url)
            .header("Authorization", format!("QQBot {}", self.access_token))
            .bearer_auth(&self.access_token)
            .json(&json_payload)
            .send()
            .await;

        println!("Response: {:#?}", resp);
        
        match resp {
            Ok(response) => {
                let status = response.status();
                let resp_text = response.text().await.unwrap_or_default();
                tracing::info!("Test message sent. Status: {}, Response: {}", status, resp_text);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to send test message: {}", e);
                Err(anyhow::anyhow!("Failed to send test message: {}", e))
            }
        }
    }
}