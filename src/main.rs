use rinko_frontend::frontend::qq;
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

            let qq_cfg_for_test = qq_cfg_shared.clone();
            // Test sending a message after initialization
            tokio::spawn(async move {
                let cfg = qq_cfg_for_test.read().await;
                if let Err(e) = cfg.test_send_message().await {
                    tracing::error!("Failed to send test message: {}", e);
                }
            });
            
            // Keep the program running to allow the background task to work
            // In a real application, you'd have your main bot logic here
            tokio::signal::ctrl_c().await?;
            tracing::info!("Shutdown signal received.");
        }
    }

    Ok(())
}