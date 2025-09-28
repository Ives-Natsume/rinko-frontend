use std::process::Command;
use crate::{
    app_status::AppStatus,
    fs::handler::{FileRequest, FileFormat, FileData},
    response::{ApiResponse, DataType},
    app_config::Config,
    CONFIG_FILE_PATH
};

pub async fn execute_command(command: &str, args: &str, app_status: &AppStatus) -> ApiResponse<Vec<String>> {
    let mut response = ApiResponse::<Vec<String>> {
        success: false,
        message: None,
        data: None,
        data_type: DataType::Text,
    };

    // possible commands:
    // - "exe <command>": execute a shell command
    // - "exe update": update config
    match args {
        "update" => {
            match config_update(app_status).await {
                Ok(_) => {
                    response.success = true;
                    response.message = Some("配置文件更新成功".to_string());
                }
                Err(e) => {
                    response.message = Some(format!("{:#?}", e));
                }
            };
            return response;
        }
        _ => {}
    }

    if cfg!(target_os = "windows") {
        response.message = Some("Windows下不支持命令执行".to_string());
    } else {
        // Unix-specific command execution
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .arg(args)
            .output()
            .map_err(|e| {
                tracing::error!("命令执行失败: {}", e);
                ApiResponse::<Vec<String>>::error(format!("命令执行失败: {:#?}", e))
            });

        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let mut results = Vec::new();
                    if !stdout.is_empty() {
                        results.push(stdout.to_string());
                    }
                    if !stderr.is_empty() {
                        results.push(stderr.to_string());
                    }
                    response.success = true;
                    response.data = Some(results);
                } else {
                    let error_message = format!(
                        "命令失败，状态: {}",
                        output.status
                    );
                    tracing::error!("{}", error_message);
                    response.message = Some(error_message);
                }
            }
            Err(e) => {
                response = e.into();
            }
        }
    }
    response
}

async fn config_update(app_status: &AppStatus) -> anyhow::Result<()> {
    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
    let request = FileRequest::Read {
        path: CONFIG_FILE_PATH.into(),
        format: FileFormat::Json,
        responder: resp_tx,
    };

    if let Err(e) = app_status.file_tx.send(request).await {
        tracing::error!("请求读取配置文件失败: {:#?}", e);
        return Err(anyhow::anyhow!("请求读取配置文件失败: {:#?}", e));
    }

    let file_result = match resp_rx.await {
        Ok(result) => result,
        Err(recv_error) => {
            tracing::error!("接收配置文件数据失败: {:#?}", recv_error);
            return Err(anyhow::anyhow!("接收配置文件数据失败: {:#?}", recv_error));
        }
    };

    let config_data = match file_result {
        Ok(FileData::Json(data)) => data,
        Ok(_) => {
            tracing::error!("收到非期望的文件格式");
            return Err(anyhow::anyhow!("收到非期望的文件格式"));
        },
        Err(file_error) => {
            tracing::error!("文件操作失败: {:#?}", file_error);
            return Err(anyhow::anyhow!("文件操作失败: {:#?}", file_error));
        },
    };

    let config: Config = serde_json::from_value(config_data)
        .map_err(|e| anyhow::anyhow!("配置文件解析失败: {:#?}", e))?;

    // Update the app status with the new config
    app_status.update_config(config).await;

    Ok(())
}