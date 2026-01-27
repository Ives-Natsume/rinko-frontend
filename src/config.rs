use std::sync::OnceLock;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub token: String,
    pub guild_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QQConfig {
    
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
pub enum BotInstanceConfig {
    Discord(DiscordConfig),
    QQ(QQConfig),
    Telegram(TelegramConfig),
    EnterpriseWeChat(EnterpriseWeChatConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub instances: Vec<BotInstanceConfig>,
}

pub static CONFIG: OnceLock<GlobalConfig> = OnceLock::new();
    
pub fn read_config() -> anyhow::Result<()> {
    let path = "config.toml";
    let config_str = std::fs::read_to_string(path)?;
    let config: GlobalConfig = match toml::from_str(&config_str) {
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
        println!("Config: {:?}", config);
        assert!(matches!(config.lidar.data_source, DataSource::Udp | DataSource::Ros));
    }
}