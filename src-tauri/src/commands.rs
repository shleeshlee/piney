use futures_util::StreamExt;
use std::collections::HashMap;
use std::io::Write;
use tauri::{command, AppHandle, Manager};
use uuid::Uuid;

/// 带进度的大文件下载命令
///
/// 流式下载到临时文件，通过 Tauri 事件发送进度
/// 然后复制到目标路径（支持 Android content:// URI）
/// 最后删除临时文件
#[command]
pub async fn download_with_progress(
    app: AppHandle,
    url: String,
    headers: Option<HashMap<String, String>>,
    target_path: String,
    method: Option<String>,
    body: Option<String>,
) -> Result<(), String> {
    let client = reqwest::Client::new();

    // 根据 method 构建请求（默认 GET）
    let method_str = method.as_deref().unwrap_or("GET");
    let mut request = match method_str.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };

    // 添加自定义 headers（认证等）
    if let Some(hdrs) = headers {
        for (key, value) in hdrs {
            request = request.header(&key, &value);
        }
    }

    // 添加请求体（用于 POST 等）
    if let Some(body_str) = body {
        request = request.body(body_str);
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("服务器返回错误: {}", response.status()));
    }

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
    let mut stream = response.bytes_stream();

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
    drop(file); // 关闭文件句柄

    // 复制到目标路径
    copy_to_target(&app, &temp_path, &target_path)?;

    // 清理临时文件
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}

/// 复制文件到目标路径（跨平台支持）
/// Android 公共存储路径会触发 MediaScanner 更新文件大小显示
#[allow(unused_variables)]
fn copy_to_target(app: &AppHandle, src: &std::path::Path, target: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, PublicDir};

        let android_fs = app.android_fs();

        // 创建新文件 (自动创建目录)
        // target 此时是相对路径 "Piney/filename"
        let new_file = android_fs
            .public_storage()
            .create_new_file(None, PublicDir::Download, target, None)
            .map_err(|e| format!("创建文件失败: {}", e))?;

        // 打开目标文件流
        let mut dest_file = android_fs
            .open_file_writable(&new_file)
            .map_err(|e| format!("打开目标文件失败: {}", e))?;

        // 打开源文件
        let mut src_file =
            std::fs::File::open(src).map_err(|e| format!("打开源文件失败: {}", e))?;

        // 流式复制
        std::io::copy(&mut src_file, &mut dest_file).map_err(|e| format!("复制文件失败: {}", e))?;

        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        use std::path::Path;

        let target_path = Path::new(target);

        // 确保目标目录存在
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
            }
        }

        // 使用标准文件复制
        std::fs::copy(src, target).map_err(|e| format!("复制文件失败: {}", e))?;

        Ok(())
    }
}

/// 简单下载命令（兼容旧接口，用于小文件）
/// 返回字节数据，适合小文件直接传回前端
#[command]
pub async fn download_large_file(
    url: String,
    headers: Option<HashMap<String, String>>,
    method: Option<String>,
    body: Option<String>,
) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::new();

    // 根据 method 构建请求（默认 GET）
    let method_str = method.as_deref().unwrap_or("GET");
    let mut request = match method_str.to_uppercase().as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    };

    if let Some(hdrs) = headers {
        for (key, value) in hdrs {
            request = request.header(&key, &value);
        }
    }

    // 添加请求体（用于 POST 等）
    if let Some(body_str) = body {
        request = request.body(body_str);
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

/// 写入数据到 Android 公共存储并触发媒体扫描
/// 用于前端直接写入文件内容到公共存储目录（如 Download/Piney/）
/// 写入后自动触发 MediaScanner，解决文件大小显示为 0B 的问题
#[command]
#[allow(unused_variables)]
pub async fn write_to_android_public(
    app: AppHandle,
    target_path: String,
    data: Vec<u8>,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, PublicDir};

        let android_fs = app.android_fs();

        // 创建新文件 (如果父目录不存在会自动创建)
        // target_path 此时应该是相对路径，例如 "Piney/image.png"
        let new_file = android_fs
            .public_storage()
            .create_new_file(
                None, // 使用默认卷
                PublicDir::Download,
                &target_path,
                None, // 让系统自动检测 mime type
            )
            .map_err(|e| format!("创建文件失败: {}", e))?;

        // 写入数据
        android_fs
            .write(&new_file, &data)
            .map_err(|e| format!("写入失败: {}", e))?;
    }

    #[cfg(not(target_os = "android"))]
    {
        // 非 Android 平台使用标准写入
        std::fs::write(&target_path, &data).map_err(|e| format!("写入失败: {}", e))?;
    }

    Ok(())
}

#[command]
pub async fn restart_server(
    state: tauri::State<'_, std::sync::Arc<crate::ServerControl>>,
) -> Result<(), String> {
    state.restart_trigger.notify_one();
    Ok(())
}
