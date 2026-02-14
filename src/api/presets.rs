use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json},
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::api::dashboard::invalidate_cache;
use crate::entities::preset;

// --- DTO ---

#[derive(Serialize)]
pub struct PresetListItem {
    pub id: Uuid,
    pub title: String,
    pub has_regex: bool,
    pub user_note: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct UpdatePresetSchema {
    pub title: Option<String>,
    pub data: Option<Value>,
    pub regex_data: Option<Value>,
    pub user_note: Option<String>,
    pub pipi_study: Option<String>,
    pub version: Option<String>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub file_name: String,
    pub status: String,
    pub reason: Option<String>,
}

// --- 导入 ---
pub async fn import(
    State(db): State<DatabaseConnection>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut results = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("unknown.json").to_string();

        // 读取字节
        let data = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                results.push(ImportResult {
                    file_name,
                    status: "error".to_string(),
                    reason: Some(e.to_string()),
                });
                continue;
            }
        };

        // 解析 JSON
        let json_string = match String::from_utf8(data.to_vec()) {
            Ok(s) => s,
            Err(_) => {
                results.push(ImportResult {
                    file_name,
                    status: "error".to_string(),
                    reason: Some("无效的编码".to_string()),
                });
                continue;
            }
        };

        let json_data: Value = match serde_json::from_str(&json_string) {
            Ok(v) => v,
            Err(e) => {
                results.push(ImportResult {
                    file_name,
                    status: "error".to_string(),
                    reason: Some(format!("无效的 JSON: {}", e)),
                });
                continue;
            }
        };

        // 导入检测：必须同时包含 temp、top_k、top_p、rep_pen
        // 导入检测：检查是否包含常见的预设字段
        // 支持全名和缩写，只要满足其中一种组合即可
        let has_temp = json_data.get("temp").is_some() || json_data.get("temperature").is_some();
        let has_top_p = json_data.get("top_p").is_some();
        let has_rep_pen =
            json_data.get("rep_pen").is_some() || json_data.get("repetition_penalty").is_some();

        if !has_temp || !has_top_p || !has_rep_pen {
            results.push(ImportResult {
                file_name,
                status: "error".to_string(),
                reason: Some(
                    "可能不是预设文件，或是不受支持的预设文件，本功能仅支持聊天补全预设"
                        .to_string(),
                ),
            });
            continue;
        }

        // 提取标题（文件名去扩展名）
        let title = std::path::Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("导入的预设")
            .to_string();

        // 提取 extensions.regex_scripts 作为 regex_data
        let regex_data = json_data
            .get("extensions")
            .and_then(|ext| ext.get("regex_scripts"))
            .cloned()
            .unwrap_or(Value::Array(vec![]));

        let regex_data_str =
            serde_json::to_string_pretty(&regex_data).unwrap_or_else(|_| "[]".to_string());

        // 格式化 JSON 存储
        let pretty_json_str = serde_json::to_string_pretty(&json_data).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("格式化 JSON 失败: {}", e),
            )
        })?;

        let uuid = Uuid::new_v4();
        let now = chrono::Utc::now().naive_utc();

        let active_model = preset::ActiveModel {
            id: Set(uuid),
            title: Set(title),
            data: Set(pretty_json_str),
            regex_data: Set(regex_data_str),
            user_note: Set(String::new()),
            pipi_study: Set(String::new()),
            version: Set("1.0.0".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        };

        match active_model.insert(&db).await {
            Ok(_) => {
                results.push(ImportResult {
                    file_name,
                    status: "success".to_string(),
                    reason: None,
                });
            }
            Err(e) => {
                results.push(ImportResult {
                    file_name,
                    status: "error".to_string(),
                    reason: Some(format!("数据库错误: {}", e)),
                });
            }
        }
    }

    if results.iter().any(|r| r.status == "success") {
        invalidate_cache();
    }
    Ok(Json(results))
}

// --- 列表 ---
pub async fn list(
    State(db): State<DatabaseConnection>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let items = preset::Entity::find()
        .order_by_desc(preset::Column::UpdatedAt)
        .all(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let list_items: Vec<PresetListItem> = items
        .into_iter()
        .map(|item| {
            // 检查 regex_data 是否为非空数组
            let has_regex = serde_json::from_str::<Value>(&item.regex_data)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .map(|arr| !arr.is_empty())
                .unwrap_or(false);

            PresetListItem {
                id: item.id,
                title: item.title,
                has_regex,
                user_note: item.user_note,
                created_at: item.created_at.and_utc().to_rfc3339(),
                updated_at: item.updated_at.and_utc().to_rfc3339(),
                version: item.version,
            }
        })
        .collect();

    Ok(Json(list_items))
}

// --- 详情 ---
pub async fn get_details(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let item = preset::Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "预设不存在".to_string()))?;

    Ok(Json(item))
}

// --- 更新 ---
pub async fn update(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdatePresetSchema>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let mut item: preset::ActiveModel = preset::Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "预设不存在".to_string()))?
        .into();

    if let Some(title) = payload.title {
        item.title = Set(title);
    }

    if let Some(user_note) = payload.user_note {
        item.user_note = Set(user_note);
    }

    if let Some(data) = payload.data {
        let json_str = serde_json::to_string_pretty(&data)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        item.data = Set(json_str);
    }

    if let Some(regex_data) = payload.regex_data {
        let json_str = serde_json::to_string_pretty(&regex_data)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        item.regex_data = Set(json_str);
    }

    if let Some(pipi_study) = payload.pipi_study {
        item.pipi_study = Set(pipi_study);
    }

    if let Some(version) = payload.version {
        item.version = Set(version);
    }

    item.updated_at = Set(chrono::Utc::now().naive_utc());

    let updated = item
        .update(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    invalidate_cache();
    Ok(Json(updated))
}

// --- 删除 ---
pub async fn delete(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    preset::Entity::delete_by_id(id)
        .exec(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    invalidate_cache();
    Ok(StatusCode::NO_CONTENT)
}

// --- 导出预设（包含正则）---
pub async fn export(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let item = preset::Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "预设不存在".to_string()))?;

    // 构建安全文件名 (RFC 5987)
    let safe_name = item
        .title
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let filename_utf8 = utf8_percent_encode(&safe_name, NON_ALPHANUMERIC).to_string();
    let content_disposition = format!("attachment; filename*=UTF-8''{}.json", filename_utf8);

    // 强制格式化 JSON (即使数据库中也是格式化的，确保万无一失)
    let json_data: Value = serde_json::from_str(&item.data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON 解析失败: {}", e),
        )
    })?;
    let pretty_json = serde_json::to_string_pretty(&json_data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON 格式化失败: {}", e),
        )
    })?;

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_string(),
            ),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        pretty_json,
    ))
}

// --- 仅导出配套正则 ---
pub async fn export_regex(
    State(db): State<DatabaseConnection>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let item = preset::Entity::find_by_id(id)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "预设不存在".to_string()))?;

    let safe_name = item
        .title
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let filename_utf8 = utf8_percent_encode(&safe_name, NON_ALPHANUMERIC).to_string();
    let content_disposition = format!("attachment; filename*=UTF-8''{}_regex.json", filename_utf8);

    // 强制格式化
    let json_data: Value = serde_json::from_str(&item.regex_data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON 解析失败: {}", e),
        )
    })?;
    let pretty_json = serde_json::to_string_pretty(&json_data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON 格式化失败: {}", e),
        )
    })?;

    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_string(),
            ),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        pretty_json,
    ))
}
