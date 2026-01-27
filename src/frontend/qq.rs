use crate::{config::QQConfig, utils::BotAdapter};
use crate::utils::*;
use uuid::Uuid;
use async_trait::async_trait;

#[async_trait]
impl BotAdapter for QQConfig {
    async fn process_message(&self) -> anyhow::Result<UnifiedMessage> {
        // Implementation for processing a message from QQ
        let message = UnifiedMessage {
            enevt_id: Uuid::new_v7(),
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