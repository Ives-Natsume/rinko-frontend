use reqwest;
use serde_json;
use crate::app_config;
use crate::{
    response::ApiResponse,
    i18n,
};
use crate::msg::prelude::*;

pub async fn group_msg_router(
    payload: &MessageEvent,
    config: &app_config::Config,
    url: &String,
    handler: fn(&MessageEvent, &app_config::Config) -> ApiResponse<Vec<String>>,
) {
    let response = handler(&payload, config);
}

pub async fn send_group_msg(
    response: ApiResponse<Vec<String>>,
    payload: &MessageEvent,
    url: &String,
) {
    if response.message == Some("solar image".to_string()) || response.message == Some("image".to_string()) {
        send_picture_to_group(response, payload, url).await;
        return ;
    }
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
    config: &app_config::Config,
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

pub async fn send_picture_to_group(
    response: ApiResponse<Vec<String>>,
    payload: &MessageEvent,
    url: &String
) {
    let image_path = match response.data {
        Some(data) => {
            if data.is_empty() {
                tracing::error!("No image data provided in response");
                return;
            } else if data.len() > 1 && !data[1].is_empty() {
                send_group_msg_with_pic_and_text(payload, data[0].clone(), data[1].clone(), url).await;
                return;
            } else {
                data[0].clone()
            }
        },
        _ => {
            tracing::error!("No image data provided in response");
            return;
        }
    };

    let msg_body = serde_json::json!({
        "group_id": payload.group_id,
        "message": "[CQ:image,file={}]".replace("{}", &image_path)
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

pub async fn send_group_msg_with_pic_and_text(
    payload: &MessageEvent,
    image_path: String,
    text: String,
    url: &String
) {
    let msg_body = serde_json::json!({
        "group_id": payload.group_id,
        "message": [
            {
                "type": "text",
                "data": {
                    "text": text
                }
            },
            {
                "type": "image",
                "data": {
                    "file": image_path
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
