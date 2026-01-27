use crate::{config::QQConfig, utils::BotAdapter};
use crate::utils::*;
use uuid::Uuid;
use async_trait::async_trait;

const QQ_ACCESS_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const QQ_AUTHORIZE_URL: &str = "https://api.sgroup.qq.com";

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
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to get access token from QQ: {:?}", resp_json))
        }
    }
}