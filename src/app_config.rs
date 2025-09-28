use serde::{Deserialize, Serialize};
use config;
use crate::i18n;

pub const CONFIG_PATH: &str = "config.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub bot_config: BotConfig,
    pub backend_config: BackendConfig,
    pub pass_api_config: PassApiConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotConfig {
    pub sse_url: String,
    pub listen_addr: String,
    pub qq_id: String,
    pub group_id: Vec<u64>,
    pub admin_id: Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendConfig {
    pub timeout: u64,
    pub concurrent_limit: u64,
    /// 该参数中的群聊开放过境查询相关模块
    pub pass_predict_group_id: Option<Vec<u64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PassApiConfig {
    pub host: String,
    pub api_key: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    pub day: u32,
    pub min_elevation: u32,
}

/// Load configuration from the specified file path
pub fn load_config() -> Result<Config, String> {
    let config_ = config::Config::builder()
        .add_source(config::File::with_name(CONFIG_PATH))
        .build();

    match config_ {
        Ok(cfg) => match cfg.try_deserialize::<Config>() {
            Ok(conf) => Ok(conf),
            Err(e) => {
                let error_msg = format!("{}: {}", i18n::text("config_file_parse_error"), e);
                tracing::error!("{}", error_msg);
                Err(error_msg)
            }
        },
        Err(e) => {
            let error_msg = format!("{} {}: {}", CONFIG_PATH, i18n::text("config_file_read_error"), e);
            tracing::error!("{}", error_msg);
            Err(error_msg)
        }
    }
}