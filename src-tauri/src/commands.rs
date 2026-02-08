use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

/// 下载进度事件数据
#[derive(Clone, Serialize)]
pub struct DownloadProgress {
    /// 已下载字节数
    pub downloaded: u64,
    /// 总字节数 (0 表示未知)
    pub total: u64,
    /// 百分比 (0-100, 如果 total 未知则为 0)
    pub percent: u8,
}

/// 带进度的大文件下载命令
///
/// 流式下载到临时文件，通过 Tauri 事件发送进度，返回临时文件路径
/// 前端需要使用 Tauri fs 插件的 copyFile 复制到目标路径（兼容 Android content:// URI）
#[command]
pub async fn download_with_progress(
    app: AppHandle,
    url: String,
    headers: Option<HashMap<String, String>>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut request = client.get(&url);

    // 添加自定义 headers（认证等）
    if let Some(hdrs) = headers {
        for (key, value) in hdrs {
            request = request.header(&key, &value);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("服务器返回错误: {}", response.status()));
    }

    // 获取 Content-Length（如果有）
    let total_size = response.content_length().unwrap_or(0);

    // 创建临时文件路径
    let temp_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法获取应用数据目录: {}", e))?;

    let temp_filename = format!("temp_download_{}.tmp", Uuid::new_v4());
    let temp_path = temp_dir.join(&temp_filename);

    // 确保目录存在
    if !temp_dir.exists() {
        std::fs::create_dir_all(&temp_dir).map_err(|e| format!("无法创建临时目录: {}", e))?;
    }

    // 创建临时文件
    let mut file = std::fs::File::create(&temp_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            "存储权限不足，请检查应用权限设置".to_string()
        } else if e.to_string().contains("No space") || e.to_string().contains("ENOSPC") {
            "存储空间不足，请清理后重试".to_string()
        } else {
            format!("无法创建临时文件: {}", e)
        }
    })?;

    // 流式读取并写入
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut last_percent: u8 = 0;

    let result: Result<(), String> = async {
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| format!("下载中断: {}", e))?;

            file.write_all(&chunk).map_err(|e| {
                if e.to_string().contains("No space") || e.to_string().contains("ENOSPC") {
                    "存储空间不足，下载已中断".to_string()
                } else {
                    format!("写入文件失败: {}", e)
                }
            })?;

            downloaded += chunk.len() as u64;

            // 计算进度并发送事件（避免发送过于频繁）
            let percent = if total_size > 0 {
                ((downloaded * 100) / total_size).min(100) as u8
            } else {
                0
            };

            // 只在进度变化时发送事件
            if percent != last_percent || downloaded == total_size {
                last_percent = percent;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgress {
                        downloaded,
                        total: total_size,
                        percent,
                    },
                );
            }
        }
        Ok(())
    }
    .await;

    // 如果下载失败，清理临时文件
    if let Err(e) = result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    // 确保文件完全写入磁盘
    file.sync_all()
        .map_err(|e| format!("同步文件失败: {}", e))?;

    Ok(temp_path.to_string_lossy().to_string())
}

/// 简单下载命令（兼容旧接口，用于小文件）
/// 返回字节数据，适合小文件直接传回前端
#[command]
pub async fn download_large_file(
    url: String,
    headers: Option<HashMap<String, String>>,
) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::new();
    let mut request = client.get(&url);

    if let Some(hdrs) = headers {
        for (key, value) in hdrs {
            request = request.header(&key, &value);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("服务器返回错误: {}", response.status()));
    }

    let mut data = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("读取数据失败: {}", e))?;
        data.extend_from_slice(&chunk);
    }

    Ok(data)
}

#[command]
pub async fn restart_server(
    state: tauri::State<'_, std::sync::Arc<crate::ServerControl>>,
) -> Result<(), String> {
    state.restart_trigger.notify_one();
    Ok(())
}
