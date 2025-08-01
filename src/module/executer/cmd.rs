use std::process::Command;
use crate::{
    response::ApiResponse,
};

pub async fn execute_command(command: &str, args: &str) -> ApiResponse<Vec<String>> {
    let mut response = ApiResponse::<Vec<String>> {
        success: false,
        message: None,
        data: None,
    };
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