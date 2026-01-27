use rinko_frontend::logging;
use rinko_frontend::config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    config::read_config()?;
    let _logging_guard = logging::init_logging("logs", "rinko-frontend", &config::CONFIG.get().unwrap().log_level);

    tracing::info!("Rinko Frontend started.");

    // test: init QQ bot
    if let Some(mut qq_cfg) = rinko_frontend::config::CONFIG.get().unwrap().qq.clone() {
        if let Err(e) = qq_cfg.init().await {
            tracing::error!("Failed to initialize QQ bot: {}", e);
        } else {
            tracing::info!("QQ bot initialized successfully.");
        }
    }

    Ok(())
}