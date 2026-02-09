use async_trait::async_trait;
use uuid::Uuid;
use crate::config::BotConfigs;

#[derive(Debug, Clone)]
pub struct UnifiedMessage {
    pub event_id: Uuid,  // Fixed typo: enevt_id -> event_id
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
    #[allow(unused)]
    pub fn new(configs: BotConfigs) -> Self {
        let mut adapters: Vec<Box<dyn BotAdapter + Send + Sync>> = Vec::new();
        if let Some(discord_cfg) = configs.discord {
            tracing::info!("Discord bot configured, but not yet implemented.");
            // adapters.push(Box::new(discord_cfg));
        }
        if let Some(qq_cfg) = configs.qq {
            adapters.push(Box::new(qq_cfg));
        }
        if let Some(telegram_cfg) = configs.telegram {
            tracing::info!("Telegram bot configured, but not yet implemented.");
            // adapters.push(Box::new(telegram_cfg));
        }
        if let Some(wechat_cfg) = configs.enterprise_wechat {
            tracing::info!("Enterprise WeChat bot configured, but not yet implemented.");
            // adapters.push(Box::new(wechat_cfg));
        }
        Self { adapters }
    }
}