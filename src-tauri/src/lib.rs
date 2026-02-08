//! Tauri 库入口

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Notify;

pub struct ServerControl {
    pub restart_trigger: Notify,
}

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let server_control = Arc::new(ServerControl {
        restart_trigger: Notify::new(),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_android_fs::init())
        .manage(server_control.clone())
        .invoke_handler(tauri::generate_handler![
            commands::download_large_file,
            commands::download_with_progress,
            commands::write_to_content_uri,
            commands::restart_server
        ])
        .setup(move |_app| {
            std::panic::set_hook(Box::new(|info| {
                let msg = format!("Panic occurred: {:?}", info);
                let _ = std::fs::write("panic_crash.txt", msg);
            }));

            // 设置环境变量标记 Tauri 模式
            std::env::set_var("TAURI_ENV", "1");

            if let Ok(current_dir) = std::env::current_dir() {
                println!("Tauri 启动 CWD: {:?}", current_dir);
            }

            // 移动端：使用系统分配的 App Data 目录（只有这里有读写权限）
            #[cfg(mobile)]
            {
                let path = _app.path().app_data_dir().expect("无法获取 App Data 目录");
                if !path.exists() {
                    std::fs::create_dir_all(&path).expect("无法创建数据目录");
                }
                println!("移动端数据目录 DATA_DIR: {:?}", path);
                std::env::set_var("DATA_DIR", path.to_string_lossy().to_string());
            }

            // 桌面端
            #[cfg(not(mobile))]
            {
                let current_dir = std::env::current_dir().unwrap_or_default();
                let local_data = current_dir.join("data");

                let mut use_local = false;

                // 1. 尝试使用或创建当前目录下的 data (便携模式优先)
                if local_data.exists() {
                    use_local = true;
                } else {
                    // 尝试创建 ./data
                    if let Ok(_) = std::fs::create_dir(&local_data) {
                        println!("成功创建本地 data 目录: {:?}", local_data);
                        use_local = true;
                    } else {
                        println!("无法在当前目录创建 data (可能是权限不足)，将回退到 AppData");
                    }
                }

                let mut final_data_path;
                if use_local {
                    final_data_path = local_data;
                } else {
                    // 2. 回退到系统 AppData
                    final_data_path = _app
                        .path()
                        .app_data_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("data"));
                }

                // 开发环境修正：如果在 src-tauri 目录下运行
                if current_dir.ends_with("src-tauri") {
                    let parent_data = current_dir.parent().unwrap().join("data");
                    if parent_data.exists() {
                        final_data_path = parent_data;
                    }
                }

                // 3. 确保目录存在
                if !final_data_path.exists() {
                    let _ = std::fs::create_dir_all(&final_data_path);
                }

                let abs_data = std::fs::canonicalize(&final_data_path).unwrap_or(final_data_path);
                println!("最终确定的数据目录 DATA_DIR: {:?}", abs_data);
                std::env::set_var("DATA_DIR", abs_data.to_string_lossy().to_string());
            }

            let data_dir_str = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
            let log_path_clone = std::path::PathBuf::from(&data_dir_str).join("startup.log");

            // 每次启动清空日志，只保留当次启动的记录
            // 使用 AtomicBool 确保只在首次写入时清空
            use std::sync::atomic::{AtomicBool, Ordering};
            static FIRST_LOG: AtomicBool = AtomicBool::new(true);

            let log = move |msg: &str| {
                use std::io::Write;

                // 首次写入时清空文件，后续追加
                let is_first = FIRST_LOG.swap(false, Ordering::SeqCst);

                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(is_first) // 首次清空，后续不清空
                    .append(!is_first) // 首次不追加，后续追加
                    .open(&log_path_clone)
                {
                    let _ = writeln!(
                        file,
                        "[{}] {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        msg
                    );
                }
            };

            log("====================================================");
            log(&format!("Piney App v{} 启动", env!("CARGO_PKG_VERSION")));
            log(&format!("Platform: {}", std::env::consts::OS));
            log(&format!("Arch: {}", std::env::consts::ARCH));
            log(&format!(
                "DATA_DIR: {}",
                std::env::var("DATA_DIR").unwrap_or_else(|_| "未设置".to_string())
            ));
            log("====================================================");
            log("Tauri setup completed. Spawning backend thread...");

            // 启动后端服务（在单独的线程中）
            // 使用 AtomicBool 追踪后端状态：
            // - swap(true) 防止 Activity 重建导致重复启动 (和 OnceLock 效果一样)
            // - 崩溃后 store(false) 允许下次重启 (OnceLock 做不到)
            static BACKEND_RUNNING: AtomicBool = AtomicBool::new(false);

            // swap(true) 返回旧值：如果是 false，说明后端没在运行，我们启动它
            // 如果是 true，说明后端已在运行，跳过
            if !BACKEND_RUNNING.swap(true, Ordering::SeqCst) {
                let log_clone = log.clone();
                let params_control = server_control.clone(); // Capture for thread
                log("Spawning backend thread...");
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        log_clone("Backend thread started.");
                        if let Err(e) = start_backend(log_clone.clone(), params_control).await {
                            log_clone(&format!("Backend CRASHED: {}", e));
                            eprintln!("后端启动失败: {}", e);
                        } else {
                            log_clone("Backend stopped unexpectedly (or app closed).");
                        }
                        // 关键：后端退出后重置标志，允许下次 Activity 重建时重启
                        BACKEND_RUNNING.store(false, Ordering::SeqCst);
                        log_clone("Backend exited. Flag reset, will restart on next lifecycle.");
                    });
                });
            } else {
                log("[SKIP] Backend already running (Activity 重建检测). Skipping spawn.");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("运行 Tauri 应用时出错");
}

async fn start_backend<F>(log: F, control: Arc<ServerControl>) -> anyhow::Result<()>
where
    F: Fn(&str) + Clone + Send + Sync + 'static,
{
    loop {
        // 数据库初始化 (带重试逻辑，解决 Android 上的瞬时故障)
        log("Initializing database...");
        let mut db_retries = 10;
        let db = loop {
            match piney::db::init_database().await {
                Ok(d) => {
                    log("Database initialized successfully.");
                    break d;
                }
                Err(e) => {
                    db_retries -= 1;
                    if db_retries <= 0 {
                        log(&format!("Database init failed after 10 retries: {}", e));
                        return Err(e.into());
                    }
                    log(&format!(
                        "Database init failed: {}, retrying in 1s... ({} retries left)",
                        e, db_retries
                    ));
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        };

        // 2. 运行模式
        let mode = piney::utils::mode_detect::RunMode::App;

        // 3. 初始化 Config
        let config_path = piney::utils::paths::get_data_path("config.yml");
        log(&format!("Loading config from: {:?}", config_path));

        let config = piney::config::ConfigState::new(&config_path.to_string_lossy());
        if config.is_initialized() {
            log("Config loaded: 已初始化 (用户已注册)");
        } else {
            log("Config loaded: 未初始化 (需要注册)");
        }

        // 4. 创建 Axum 应用
        log("Creating Axum app...");
        let app = piney::create_app(db, mode, config).await;

        // 5. 启动侦听
        let port = 9696;
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));

        log(&format!("Binding to address: {}", addr));

        let mut port_retries = 0;
        let listener = loop {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    if port_retries > 0 {
                        log(&format!(
                            "Port {} bound successfully after {} retries",
                            port, port_retries
                        ));
                    }
                    break l;
                }
                Err(e) => {
                    port_retries += 1;
                    log(&format!(
                        "[RETRY {}] Failed to bind port {}: {}, retrying in 1s...",
                        port_retries, port, e
                    ));
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        };

        log("Server listening. Entering loop...");

        // 优雅关闭信号
        let shutdown_signal = {
            let control = control.clone();
            let log = log.clone();
            async move {
                control.restart_trigger.notified().await;
                log("Received restart signal. Shutting down Axum...");
            }
        };

        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal)
            .await
        {
            log(&format!("Server error: {}", e));
            // 如果是严重错误可能需要退出，但这里假设是重启信号或临时错误
        }

        log("Axum server stopped. Restarting in 1s...");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
