extern crate lazy_static;
mod app_status;
mod i18n;
mod config;
mod logger;
mod response;
mod web;
mod msg;
mod socket;
mod fs;
mod module;
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

    // tokio::spawn(async move {
    //     let app = Router::new()
    //         .route("/reload-config", post(web::http_cmd::reload_config_handler))
    //         .with_state(app_status);

    //         axum_server::Server::bind("0.0.0.0:8080".parse().unwrap())
    //             .serve(app.into_make_service())
    //             .await
    //             .unwrap();
    //     }
    // );

    let app_status = socket::initialize_app_status().await;
    let listen_addr = {
        let config = app_status.config.read().await;
        config.bot_config.listen_addr.parse::<std::net::SocketAddr>()?
    };

    let shared_app_status = Arc::new(app_status);
    tokio::spawn(async move {
        if let Err(e) = web::sse::run_sse_loop(shared_app_status).await {
            tracing::error!("{}: {}", i18n::text("sse_loop_err"), e);
        }
    });

    let listener = TcpListener::bind(listen_addr).await?;
    tracing::info!("{}: {:?}", i18n::text("server_started"), listener.local_addr());

    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                tokio::spawn(async move {
                    if let Err(e) = socket::handle_connection(stream, addr).await {
                        tracing::error!("Connection error with {}: {}", addr, e);
                    }
                    tracing::info!("Connection {} closed", addr);
                });
            }
            Err(e) => {
                tracing::error!("Failed to accept connection: {}", e);
                // keep the server alive
            }
        }
    }
}
