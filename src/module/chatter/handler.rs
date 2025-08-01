use crate::msg::prelude::*;
use crate::{app_status::AppStatus, msg::group_msg};
use crate::module::chatter::commands;
use crate::response::ApiResponse;
use std::sync::Arc;
use regex;

/// Pre-process incoming messages from the LLOneBot server
pub async fn message_handler(
    data: String,
    app_status: &Arc<AppStatus>,
) {
    let config = app_status.config.read().await;
    if let Ok(payload) = parse_message_event(&data) {
        for elem in &payload.message.clone() {
            match elem {
                MessageElement::Text { .. } => {
                    text_router(&payload, app_status).await;
                }
                MessageElement::At { qq, .. } => {
                    if *qq == config.bot_config.qq_id {
                        text_router(&payload, app_status).await;
                    }
                    break;  // exit early if we find an @ mention
                }
                _ => {}
            }
        }
    }
}

/// Route text messages to appropriate handlers
async fn text_router(
    payload: &MessageEvent,
    app_status: &Arc<AppStatus>,
) {
    let mut message_text: String = String::new();
    for elem in &payload.message {
        if let MessageElement::Text { text } = elem {
            message_text.push_str(text);
        }
    }

    // remove leading and trailing whitespace
    let text = message_text.trim().to_string();

    if text.starts_with("/") {
        let (command, args) = get_command_and_args(&text);
        if !command.is_empty() {
            command_handler(&command, &args, payload, app_status).await;
        } else {
            return ;
        }
    } else {
        let normalized_text = string_normalize(&text);
        text_handler(&normalized_text, payload, app_status).await;
    }
}

async fn command_handler(
    command: &str,
    args: &str,
    payload: &MessageEvent,
    app_status: &Arc<AppStatus>
) {
    // remove all punctuation
    let command = command.trim_matches(|c: char| c.is_ascii_punctuation()).to_lowercase();
    let args = args.trim_matches(|c: char| c.is_ascii_punctuation()).to_string();

    let response = match commands::process_command(&command, &args, app_status, payload).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Command processing error: {:?}", e);
            ApiResponse::error(format!("Rinko发生了内部异常喵>_\n{:#?}", e))
        }
    };

    if response != ApiResponse::empty(){
        let url = app_status.config.read().await.bot_config.url.clone();
        group_msg::send_group_msg(response, payload, &url).await;
    }
}

async fn text_handler(
    text: &Vec<String>,
    payload: &MessageEvent,
    app_status: &Arc<AppStatus>,
) {
    let response = match commands::process_text(text.clone(), app_status).await {
        Ok(resp) => resp,
        Err(e) => {
            tracing::error!("Text processing error: {:?}", e);
            return ;
        }
    };
    if response != ApiResponse::empty(){
        let url = app_status.config.read().await.bot_config.url.clone();
        group_msg::send_group_msg(response, payload, &url).await;
    }
}

/// - Split by whitespace and normalize
/// - Keep CJK characters intact
pub fn string_normalize(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .map(|s| s.to_lowercase())
        .collect()
}

/// Get the command and arguments from the input string
/// - Supports commands like `/command args`
/// - Returns a tuple of (command, args)
pub fn get_command_and_args(input: &str) -> (String, String) {
    let re = regex::Regex::new(r"^\s*/(\w+)(?:\s+([\s\S]*))?$").unwrap();
    if let Some(caps) = re.captures(input) {
        let command = caps.get(1).map_or("", |m| m.as_str());
        let args = caps.get(2).map_or("", |m| m.as_str());
        (command.to_string(), args.to_string())
    } else {
        (String::new(), String::new())
    }
}