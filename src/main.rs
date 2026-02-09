use rinko_frontend::logging;
use rinko_frontend::config;
use rinko_frontend::config::QQConfig;
use std::sync::Arc;
use tokio::sync::RwLock;

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
            
            // Wrap in Arc<RwLock> for thread-safe access and start auto-renewal
            let qq_cfg_shared = Arc::new(RwLock::new(qq_cfg));
            QQConfig::start_token_renewal_task(qq_cfg_shared.clone());
            tracing::info!("QQ token auto-renewal task started.");

            // Start webhook server
            let qq_cfg_for_webhook = qq_cfg_shared.clone();
            tokio::spawn(async move {
                if let Err(e) = QQConfig::start_webhook_server(qq_cfg_for_webhook, 3110).await {
                    tracing::error!("Webhook server error: {}", e);
                }
            });
            tracing::info!("QQ webhook server starting on port 3110...");
            
            // Keep the program running to allow the background task to work
            // In a real application, you'd have your main bot logic here
            tokio::signal::ctrl_c().await?;
            tracing::info!("Shutdown signal received.");
        }
    }

    Ok(())
}