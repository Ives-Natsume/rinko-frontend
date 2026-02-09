use std::sync::OnceLock;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub token: String,
    pub guild_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQConfig {
    pub app_id: String,
    pub client_secret: String,       // also used as bot_secret for webhook signature verification
    pub access_token: String,
    pub token_expires_in: u64,       // expire time in seconds
    #[serde(skip)]
    pub client: reqwest::Client,
    #[serde(skip)]
    pub token_fetched_at: Option<tokio::time::Instant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub token: String,
    pub chat_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnterpriseWeChatConfig {
    pub corp_id: String,
    pub agent_id: u32,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub enable: bool,
    pub url: String,
    pub frontend_id: String,
    pub heartbeat_interval: u64,  // in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfigs {
    pub backend: Option<BackendConfig>,
    pub discord: Option<DiscordConfig>,
    pub qq: Option<QQConfig>,
    pub telegram: Option<TelegramConfig>,
    pub enterprise_wechat: Option<EnterpriseWeChatConfig>,
    pub log_level: String,
}

pub static CONFIG: OnceLock<BotConfigs> = OnceLock::new();
    
pub fn read_config() -> anyhow::Result<()> {
    let path = "config.toml";
    let config_str = std::fs::read_to_string(path)?;
    let config: BotConfigs = match toml::from_str(&config_str) {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("Failed to parse config file {}: {}", path, e);
            panic!()
        }
    };

    CONFIG.set(config.clone()).unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {
        read_config().unwrap();
        let config = CONFIG.get().unwrap();
        println!("Loaded config: {:#?}", config);
    }
}