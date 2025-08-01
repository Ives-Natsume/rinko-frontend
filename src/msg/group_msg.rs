use reqwest;
use serde_json;
use crate::config;
use crate::{
    response::ApiResponse,
    i18n,
};
use crate::msg::prelude::*;

pub async fn send_group_msg(
    response: ApiResponse<Vec<String>>,
    payload: &MessageEvent,
    url: &String,
) {
    let message_text: String = response
        .data
        .map(|data| data.join("\n"))
        .unwrap_or_else(|| response.message.unwrap_or_else(|| i18n::text("no_response_data")));

    let group_id = payload.group_id;
    let msg_body = serde_json::json!({
        "group_id": group_id,
        "message": [
            {
                "type": "text",
                "data": {
                    "text": message_text
                }
            }
        ]
    });

    let endpoint_url = format!("{}/send_group_msg", url);
    let client = reqwest::Client::new();
    let response = client
        .post(endpoint_url)
        .json(&msg_body)
        .send()
        .await;

    match response {
        Ok(res) => {
            let status = res.status();
            let body = res.text().await.unwrap_or_else(|_| "<Failed to read body>".to_string());
            if !status.is_success() {
                tracing::error!("{}: {}", i18n::text("send_group_msg_err"), body);
            }
            if body.contains("error") {
                tracing::error!("{}: {}", i18n::text("send_group_msg_err"), body);
            }
        }
        Err(err) => {
            tracing::error!("{}: {}", i18n::text("send_group_msg_err"), err);
        }
    }
}

pub async fn _send_group_message_to_multiple_groups(
    response: ApiResponse<Vec<String>>,
    config: &config::Config,
    url: &String,
) {
    let groups = &config.bot_config.group_id;
    let msg_text = response
        .data
        .map(|data| data.join("\n"))
        .unwrap_or_else(|| response.message.unwrap_or_else(|| i18n::text("no_response_data")));
    for group_id in groups {
        let msg_body = serde_json::json!({
            "group_id": group_id,
            "message": [
                {
                    "type": "text",
                    "data": {
                        "text": msg_text
                    }
                }
            ]
        });

        let endpoint_url = format!("{}/send_group_msg", url);
        let client = reqwest::Client::new();
        let response = client
            .post(endpoint_url)
            .json(&msg_body)
            .send()
            .await;

        match response {
            Ok(res) => {
                let status = res.status();
                let body = res.text().await.unwrap_or_else(|_| "<Failed to read body>".to_string());
                if !status.is_success() {
                    tracing::error!("{}: {}", i18n::text("send_group_msg_err"), body);
                }
                if body.contains("error") {
                    tracing::error!("{}: {}", i18n::text("send_group_msg_err"), body);
                }
            }
            Err(err) => {
                tracing::error!("{}: {}", i18n::text("send_group_msg_err"), err);
            }
        }
    }
}
