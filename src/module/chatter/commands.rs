use serde::Deserialize;
use std::collections::HashMap;
use crate::fs::handler::{FileRequest, FileFormat, FileData};
use crate::module::executer::cmd;
use crate::msg::prelude::{IntoBinMessageEvent, MessageEvent};
use crate::response::ApiResponse;
use crate::socket::{BotMessage, MsgContent};
use crate::app_status::AppStatus;
use crate::COMMAND_TOML_PATH;
use crate::KEYWORDS_TOML_PATH;

#[derive(Debug, Deserialize)]
pub struct CommandList {
    pub commands: HashMap<String, CommandDef>,
}

#[derive(Debug, Deserialize)]
pub struct CommandDef {
    #[serde(default)]
    pub local: bool,

    #[serde(default)]
    pub value: Option<String>,

    #[serde(default)]
    pub pic: Option<String>,

    #[serde(default)]
    pub help: Option<String>,

    #[serde(default)]
    pub entertain: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct KeywordList {
    pub keywords: HashMap<String, String>,
}

pub async fn process_command(
    command: &str,
    args: &str,
    app_status: &AppStatus,
    payload: &MessageEvent
) -> anyhow::Result<ApiResponse<Vec<String>>> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let command_read_request = FileRequest::Read {
        path: COMMAND_TOML_PATH.into(),
        format: FileFormat::Toml,
        responder: resp_tx,
    };

    if let Err(e) = app_status.file_tx.send(command_read_request).await {
        let err_msg = format!("请求读取命令列表失败，请联系管理员:\n{:#?}", e);
        tracing::error!("{}", err_msg);
        return Ok(ApiResponse { success: false, message: Some(err_msg), data: None });
    }

    let file_result = match resp_rx.await {
        Ok(result) => result,
        Err(recv_error) => {
            let err_msg = format!("接收命令列表数据失败，请联系管理员:\n{:#?}", recv_error);
            tracing::error!("{}", err_msg);
            return Ok(ApiResponse { success: false, message: Some(err_msg), data: None });
        }
    };

    let command_list = match file_result {
        Ok(FileData::Toml(data)) => {
            let toml_string = toml::to_string(&data)?; // propagate as anyhow::Error
            toml::from_str::<CommandList>(&toml_string)?
        },
        Ok(_) => {
            let err_msg = "Rinko收到非期望的文件格式，请联系管理员".to_string();
            tracing::error!("{}", err_msg);
            return Ok(ApiResponse { success: false, message: Some(err_msg), data: None });
        },
        Err(file_error) => {
            let err_msg = format!("文件操作失败，请联系管理员\n{:#?}", file_error);
            tracing::error!("{}", err_msg);
            return Ok(ApiResponse { success: false, message: Some(err_msg), data: None });
        },
    };

    let mut response: ApiResponse<Vec<String>> = ApiResponse {
        success: false,
        message: None,
        data: None,
    };

    let is_help_request = args.trim() == "h" || args.trim() == "help";

    let query_id = payload.user_id.clone();
    let group_id = payload.group_id.clone();
    let config = app_status.config.read().await.bot_config.clone();
    let is_admin = config.admin_id.contains(&query_id);

    if let Some(cmd) = command_list.commands.get(command) {
        if is_help_request {
            if let Some(help_text) = &cmd.help {
                response.success = true;
                response.data = Some(vec![help_text.clone()]);
            } else {
                response.message = Some("fna没写捏^ ^)/".to_string());
            }
            return Ok(response);
        }

        if cmd.local {
            if let Some(entertain) = cmd.entertain {
                if entertain && group_id == 95339567 {
                    response = ApiResponse::empty();
                    return Ok(response);
                }
            }
            if let Some(pic) = &cmd.pic {
                response.success = true;
                response.data = Some(vec![pic.clone(), cmd.value.clone().unwrap_or_default()]);
                response.message = Some("image".to_string());
            } else if let Some(value) = &cmd.value {
                response.success = true;
                response.data = Some(vec![value.clone()]);
            } else {
                response = local_cmd_router(command, args, app_status, is_admin).await;
            }
        } else {
            let msg_content = MsgContent {
                command: Some(command.to_string()),
                payload: Some(payload.clone().into_bin_message_event()),
                message: None,
                api_response: None,
            };
            let bot_msg = BotMessage::Chat {
                from: "rinko_bot_core".to_string(),
                to: "CiRCLE_sat_bot_server".to_string(),
                content: msg_content,
            };
            if let Err(e) = app_status.send_bot_message(bot_msg).await {
                tracing::warn!("Could not send to bot: {}", e);
                let response_msg = "当前为Rinko重构版测试阶段，拓展服务器未响应喵>_".to_string();
                response.message = Some(response_msg);
            }
        }
    } else {
        response.message = Some(format!("Rinko重构版正在测试喵^ ^)/"));
    }

    Ok(response)
}

pub async fn process_text(
    text_list: Vec<String>,
    app_status: &AppStatus,
) -> anyhow::Result<ApiResponse<Vec<String>>> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let keyword_read_request = FileRequest::Read {
        path: KEYWORDS_TOML_PATH.into(),
        format: FileFormat::Toml,
        responder: resp_tx,
    };

    if let Err(e) = app_status.file_tx.send(keyword_read_request).await {
        return Err(anyhow::Error::new(e).context("Failed to send keyword read request"));
    }

    let file_result = match resp_rx.await {
        Ok(result) => result,
        Err(recv_error) => {
            return Err(anyhow::Error::new(recv_error).context("Failed to receive keyword list"));
        }
    };

    let keyword_list = match file_result {
        Ok(FileData::Toml(data)) => {
            let toml_string = toml::to_string(&data)?;
            toml::from_str::<KeywordList>(&toml_string)?
        },
        Ok(_) => {
            return Err(anyhow::Error::msg("Received unexpected file format for keywords"));
        },
        Err(file_error) => {
            return Err(anyhow::Error::msg(format!("File operation failed: {:?}", file_error)));
        },
    };

    if keyword_list.keywords.is_empty() {
        return Ok(ApiResponse::empty());
    }

    let mut response = ApiResponse::<Vec<String>>::empty();
    let mut response_data = Vec::new();
    for text in text_list {
        if let Some(keyword_response) = keyword_list.keywords.get(text.as_str()) {
            response_data.push(keyword_response.clone());
            break;
        }
    }
    
    if response_data.is_empty() {
        return Ok(response);
    }
    response.success = true;
    response.data = Some(response_data);

    Ok(response)
}

/// Local command execution router
/// The commands here all have no default value
async fn local_cmd_router(
    command: &str,
    args: &str,
    app_status: &AppStatus,
    is_admin: bool
) -> ApiResponse<Vec<String>> {
    let mut response: ApiResponse<Vec<String>> = ApiResponse {
        success: false,
        message: None,
        data: None,
    };

    match command {
        "exe" => {
            if !is_admin {
                response.message = Some("你没有权限执行此命令喵>_".to_string());
                return response;
            }
            response = cmd::execute_command(command, args, app_status).await;
        }
        _ => {}
    }

    response
}