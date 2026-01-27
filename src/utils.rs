use async_trait::async_trait;
use uuid::Uuid;
use crate::config::BotInstanceConfig;

#[derive(Debug, Clone)]
pub struct UnifiedMessage {
    pub enevt_id: Uuid,
    pub content: String,
    pub platform: Platform,
}

#[derive(Debug, Clone)]
pub enum Platform {
    QQ,
    EnterpriseWeChat,
    Telegram,
    Discord,
}

#[async_trait]
pub trait BotAdapter {
    async fn process_message(&self) -> anyhow::Result<UnifiedMessage>;
    async fn send_message(&self, msg: &UnifiedMessage) -> anyhow::Result<()>;
}

pub struct BotManager {
    // dynamic dispatch for different bot adapters
    pub adapters: Vec<Box<dyn BotAdapter + Send + Sync>>,
}

impl BotManager {
    pub fn new(configs: Vec<BotInstanceConfig>) -> Self {
        let mut adapters: Vec<Box<dyn BotAdapter + Send + Sync>> = Vec::new();
        for config in configs {
            match config {
                BotInstanceConfig::QQ(c) => adapters.push(Box::new(c)),
                _ => {}
            }
        }
        Self { adapters }
    }
}