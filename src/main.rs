extern crate lazy_static;
mod app_status;
mod i18n;
mod app_config;
mod logger;
mod response;
mod web;
mod msg;
mod socket;
mod fs;
mod module;
use config::{Config, File, Environment};
use tokio::net::TcpListener;
use std::sync::Arc;
pub const CONFIG_FILE_PATH: &str = "config.json";
pub const DOC_FILE_PATH: &str = "locales/doc.json";
pub const COMMAND_TOML_PATH: &str = "commands.toml";
pub const KEYWORDS_TOML_PATH: &str = "keywords.toml";

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    i18n::I18N.set_lang("zh");
    
    let _logger = logger::init_logging("logs", "rinko_bot_core");
    tracing::info!("{}", i18n::text("log_initialized"));

    let config_ = app_config::load_config();
    let config = match config_ {
        Ok(cfg) => cfg,
        Err(e) => {
            tracing::error!("{}: {:#?}", i18n::text("config_load_error"), e);
            return Err(anyhow::anyhow!("{}: {:#?}", i18n::text("config_load_error"), e));
        }
    };

    let initial_config = Arc::new(config);
    tracing::info!("{}", i18n::text("config_loaded"));

    let (config_tx, config_rx) = tokio::sync::watch::channel(initial_config.clone());

    // start a background task to listen for keyboard input
    tokio::spawn(async move {
        // reload config when "reload" is typed in the console
        use tokio::io::{self, AsyncBufReadExt};
        let stdin = io::stdin();
        let mut reader = io::BufReader::new(stdin).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if line.trim() == "reload" {
                match app_config::load_config() {
                    Ok(new_config) => {
                        let new_config = Arc::new(new_config);
                        if config_tx.send(new_config).is_ok() {
                            tracing::info!("{}", i18n::text("config_reloaded"));
                        } else {
                            tracing::error!("{}", i18n::text("config_reload_failed"));
                        }
                    }
                    Err(e) => {
                        tracing::error!("{}: {:#?}", i18n::text("config_load_error"), e);
                    }
                }
            }
        }
    });

    // test: start multiple tasks to read the config
    for i in 0..3 {
        let mut rx = config_rx.clone();
        tokio::spawn(async move {
            loop {
                if rx.changed().await.is_ok() {
                    let cfg = rx.borrow();
                    tracing::info!("Task {}: New config received: {:?}", i, cfg.bot_config.listen_addr);
                }
            }
        });
    }

    // keep the main task alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }

    Ok(())
}
