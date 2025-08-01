use tokio::{
    sync::{
        Semaphore,
    },
    time::{
        timeout,
        Duration,
    },
};
use std::sync::Arc;
use crate::{
    app_status::AppStatus, i18n, module::chatter, socket
};
use eventsource_client::{
    Client,
};
use futures::TryStreamExt;

/// Runs the SSE connection to receive message from LLOneBot
pub async fn run_sse_loop(
    app_status: Arc<AppStatus>,
) -> anyhow::Result<()> {
    let url = {
        let config_guard = app_status.config.read().await;
        format!("{}/_events", config_guard.bot_config.url)
    };

    #[allow(unused_mut)]
    let mut client = eventsource_client::ClientBuilder::for_url(&url)?
        .header("Accept", "text/event-stream")?
        .build();

    tracing::info!("{}: {}", i18n::text("sse_connecting"), url);
    let mut stream = client.stream();
    let shared_config = app_status.config.clone();

    let semaphore = {
        let config_guard = shared_config.read().await;
        Arc::new(Semaphore::new(config_guard.backend_config.concurrent_limit as usize))
    };

    if let Err(e) = app_status.send_bot_message(socket::BotMessage::Chat {
        from: "rinko_bot_core".to_string(),
        to: "CiRCLE_sat_bot_server".to_string(),
        content: socket::MsgContent::msg_only(i18n::text("sse_connect_success").to_string()),
    }).await {
        tracing::warn!("Could not send message to external server: {}", e);
        // let response = ApiResponse::<Vec<String>>::error(
        //     "Rinko外围服务器离线".to_string(),
        // );
        // let config = app_status.config.read().await;
        // msg::group_msg::send_group_message_to_multiple_groups(response, &config, &url);
        // Continue running SSE even if bot external server is not available
    }

    while let Some(event) = stream.try_next().await? {
        match event {
            eventsource_client::SSE::Event(evt) => {
                if evt.event_type == "message" {
                    let data = evt.data.clone();
                    let app_status_clone = app_status.clone();
                    let semaphore: Arc<Semaphore> = semaphore.clone();

                    if let Ok(permit) = semaphore.try_acquire_owned() {
                        tokio::spawn(async move {
                            let app_status = app_status_clone.clone();
                            let config = app_status.config.read().await;
                            let _permit = permit;

                            let timeout_duration = Duration::from_secs(config.backend_config.timeout);
                            if let Err(e) = timeout(timeout_duration, chatter::handler::message_handler(data, &app_status)).await {
                                tracing::error!("Timeout or error processing message: {}", e);
                            }
                        });
                    } else {
                        tracing::warn!("{}", i18n::text("sse_too_many_msgs"));
                    }
                }
            }
            eventsource_client::SSE::Comment(_) => {

            }
            eventsource_client::SSE::Connected(_) => {
                tracing::info!("{}: {}", i18n::text("sse_connect_success"), url);
            }
        }
    }

    Ok(())
}