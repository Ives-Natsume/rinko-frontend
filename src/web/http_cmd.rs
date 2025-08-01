use axum::{
    extract::State,
    response::IntoResponse,
};
use tokio::sync::oneshot;
use crate::{
    app_status::AppStatus,
    config,
    fs::handler::{FileData, FileFormat, FileRequest},
    i18n,
    response::json_response,
    CONFIG_FILE_PATH,
};

pub async fn _reload_config_handler(
    State(app_state): State<AppStatus>
) -> impl IntoResponse {
    let mut config = app_state.config.write().await;
    let (conf_resp_tx, conf_resp_rx) = oneshot::channel();

    let config_read_request = FileRequest::Read {
        path: CONFIG_FILE_PATH.into(),
        format: FileFormat::Json,
        responder: conf_resp_tx,
    };

    if let Err(e) = app_state.file_tx.send(config_read_request).await {
        let err_msg = format!("{}: {}", i18n::text("config_reload_error"), e);
        tracing::error!("{}", err_msg);
        return json_response(
            false,
            Some(err_msg),
            None,
        );
    }

    let file_result = match conf_resp_rx.await {
        Ok(result) => result,
        Err(recv_error) => {
            let err_msg = format!("{}: Failed to receive config data: {}", i18n::text("config_reload_error"), recv_error);
            tracing::error!("{}", err_msg);
            return json_response(false, Some(err_msg), None);
        }
    };

    let new_config_result = match file_result {
        Ok(FileData::Json(data)) => {
            serde_json::from_value::<config::Config>(data)
                .map_err(|e| format!("{}: Failed to parse config JSON: {}", i18n::text("config_reload_error"), e))
        },
        Ok(_) => Err(format!("{}: Unexpected file format received", i18n::text("config_reload_error"))),
        Err(file_error) => Err(format!("{}: File operation failed: {}", i18n::text("config_reload_error"), file_error)),
    };

    match new_config_result {
        Ok(new_config) => {
            *config = new_config;
            let success_data = i18n::text("config_reload_success");
            tracing::info!("{}", success_data);
            json_response(true, None, Some(success_data))
        },
        Err(err_msg) => {
            tracing::error!("{}", err_msg);
            json_response(false, Some(err_msg), None)
        },
    }
}
