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

        // 提取标题和版本号（文件名去扩展名，并尝试从中提取版本号）
        let stem = std::path::Path::new(&file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("导入的预设")
            .to_string();

        // 标题保持原样，仅提取版本号
        let title = stem.clone();
        let version = extract_version_from_filename(&stem);

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
            version: Set(version),
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

/// 从文件名中提取版本号（仅提取版本号，不修改标题）
///
/// 支持的格式：
/// - 预设名称v1.2.3, 预设名称V1.2.3
/// - 预设名称ver1.2.3, 预设名称VER1.2.3
/// - 预设名称version1.2.3, 预设名称VERSION1.2.3
/// - 预设名称v_1.23, 预设名称ver_1.23, 预设名称version_1.23
/// - 预设名称_1.2.3, 预设名称 1.2.3, 预设名称-1.2.3
/// - 预设名称1_2_3 (下划线分隔的版本)
/// - 预设名称_1.2.3_ver, 预设名称_1.2.3_version
/// - 预设名称_1.2.3_final (版本后有其他文字)
/// - 1.2.3 (纯版本号)
/// - 321-1.2.3 (数字名称 + 版本号)
///
/// 返回提取到的版本号，若未找到返回 "1.0.0"
pub fn extract_version_from_filename(stem: &str) -> String {
    let s = stem.trim();

    // 策略1: 匹配显式版本标记 (version/ver/v，不区分大小写)
    if let Some(ver) = find_version_with_marker(s) {
        return ver;
    }

    // 策略2: 匹配尾部版本标记 (如 _ver, _version)
    if let Some(ver) = find_trailing_ver_marker(s) {
        return ver;
    }

    // 策略3: 匹配尾部点分隔版本号 (如 预设名称_1.2.3)
    if let Some(ver) = find_trailing_dotted_version(s) {
        return ver;
    }

    // 策略4: 匹配尾部下划线分隔版本号 (如 预设名称1_2_3)
    if let Some(ver) = find_trailing_underscored_version(s) {
        return ver;
    }

    // 策略5: 扫描中间位置的点分隔版本号 (如 预设名称_1.2.3_final)
    // 需要前后都有分隔符
    if let Some(ver) = find_embedded_dotted_version(s) {
        return ver;
    }

    // 策略6: 整个文件名就是版本号 (如 1.2.3)
    if is_valid_version(s) {
        return s.to_string();
    }

    // 没有匹配到版本号，使用默认值
    "1.0.0".to_string()
}

/// 版本标记列表（按长度从长到短排列，确保 "version" 优先于 "ver" 优先于 "v"）
const VERSION_MARKERS: &[&str] = &["version", "ver", "v"];

/// 尾部版本标记列表
const TRAILING_VERSION_MARKERS: &[&str] =
    &["_version", "-version", " version", "_ver", "-ver", " ver"];

/// 策略1: 显式版本标记
fn find_version_with_marker(s: &str) -> Option<String> {
    let lower = s.to_lowercase();

    for marker in VERSION_MARKERS {
        if let Some(pos) = lower.rfind(marker) {
            let after_marker = pos + marker.len();

            // 先尝试从标记后面提取版本号（如 v1.2.3, ver1.2.3）
            if after_marker < s.len() {
                let rest = &s[after_marker..];
                // 跳过可选的 _ - 空格分隔符
                let version_start =
                    if rest.starts_with('_') || rest.starts_with('-') || rest.starts_with(' ') {
                        1
                    } else {
                        0
                    };

                if version_start < rest.len() {
                    let version_part = &rest[version_start..];

                    if version_part.starts_with(|c: char| c.is_ascii_digit()) {
                        // 提取版本号（数字和点组成，遇到其他字符停止）
                        let version_end = version_part
                            .find(|c: char| !c.is_ascii_digit() && c != '.')
                            .unwrap_or(version_part.len());

                        let raw_version = &version_part[..version_end];
                        let version = normalize_version(raw_version);

                        if is_valid_version(&version) {
                            return Some(version);
                        }
                    }
                }
            }

            // 标记后面没有有效版本号，尝试从标记前面提取（如 0.0521ver, 1.2.3version）
            if pos > 0 {
                let before = &s[..pos];
                if let Some(ver) = find_trailing_dotted_version(before) {
                    return Some(ver);
                }
            }
        }
    }

    None
}

/// 策略2: 尾部版本标记
fn find_trailing_ver_marker(s: &str) -> Option<String> {
    let lower = s.to_lowercase();

    for marker in TRAILING_VERSION_MARKERS {
        if lower.ends_with(marker) {
            let without_marker = &s[..s.len() - marker.len()];
            let trimmed = without_marker.trim_end();

            if let Some(ver) = find_trailing_dotted_version(trimmed) {
                return Some(ver);
            }
            if let Some(ver) = find_trailing_underscored_version(trimmed) {
                return Some(ver);
            }
        }
    }

    None
}

/// 策略3: 尾部点分隔版本号
fn find_trailing_dotted_version(s: &str) -> Option<String> {
    let len = s.len();
    let mut i = len;
    let mut dot_count = 0;
    let mut has_digit = false;

    while i > 0 {
        let c = s[..i].chars().last().unwrap();
        let c_len = c.len_utf8();

        if c.is_ascii_digit() {
            has_digit = true;
            i -= c_len;
        } else if c == '.' && has_digit {
            dot_count += 1;
            has_digit = false;
            i -= c_len;
        } else {
            break;
        }
    }

    if dot_count == 0 {
        return None;
    }

    let version_str = &s[i..];
    if !version_str.ends_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let version = normalize_version(version_str);
    if !is_valid_version(&version) {
        return None;
    }

    if i == 0 {
        return Some(version);
    }

    let before = s[..i].chars().last().unwrap();
    if before == '_' || before == ' ' || before == '-' || !before.is_ascii() {
        return Some(version);
    }

    None
}

/// 策略4: 尾部下划线分隔版本号
fn find_trailing_underscored_version(s: &str) -> Option<String> {
    let len = s.len();
    let mut i = len;
    let mut underscore_count = 0;
    let mut has_digit = false;

    while i > 0 {
        let c = s[..i].chars().last().unwrap();
        let c_len = c.len_utf8();

        if c.is_ascii_digit() {
            has_digit = true;
            i -= c_len;
        } else if c == '_' && has_digit {
            underscore_count += 1;
            has_digit = false;
            i -= c_len;
        } else {
            break;
        }
    }

    if underscore_count == 0 {
        return None;
    }

    let raw = &s[i..];
    if !raw.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    let version = normalize_version(raw);
    if !is_valid_version(&version) {
        return None;
    }

    if i == 0 {
        return Some(version);
    }

    let before = s[..i].chars().last().unwrap();
    if !before.is_ascii_digit() {
        return Some(version);
    }

    None
}

/// 策略5: 扫描字符串中间的点分隔版本号
/// 处理如 预设名称_1.2.3_final, 预设名称-1.2.3-beta 等情况
fn find_embedded_dotted_version(s: &str) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // 找到一个分隔符后面的数字位置
        let digit_pos = if i == 0 && chars[0].is_ascii_digit() {
            // 字符串开头就是数字
            0
        } else if is_version_boundary(chars[i]) && i + 1 < len && chars[i + 1].is_ascii_digit() {
            // 分隔符后面跟数字
            i + 1
        } else {
            i += 1;
            continue;
        };

        // 如果不在开头，前面必须有分隔符或非ASCII字符
        if digit_pos > 0 && !is_version_boundary(chars[digit_pos - 1]) {
            i += 1;
            continue;
        }

        // 从这里开始尝试提取版本号
        let mut j = digit_pos;
        let mut dot_count = 0;
        let mut last_was_dot = false;

        while j < len {
            if chars[j].is_ascii_digit() {
                last_was_dot = false;
                j += 1;
            } else if chars[j] == '.' && !last_was_dot && j > digit_pos {
                dot_count += 1;
                last_was_dot = true;
                j += 1;
            } else {
                break;
            }
        }

        // 版本号必须以数字结尾，且至少有一个点
        if dot_count > 0 && !last_was_dot {
            // 版本号后面必须是分隔符、非ASCII字符或字符串结尾
            if j == len || is_separator(chars[j]) || !chars[j].is_ascii() {
                let version_str: String = chars[digit_pos..j].iter().collect();
                let version = normalize_version(&version_str);
                if is_valid_version(&version) {
                    return Some(version);
                }
            }
        }

        // 跳过已扫描的部分，避免无限循环
        i = if j > i + 1 { j } else { i + 1 };
    }

    None
}

/// 判断字符是否为版本号分隔符
fn is_separator(c: char) -> bool {
    c == '_' || c == ' ' || c == '-'
}

/// 判断字符是否可以作为版本号的边界（分隔符或非ASCII字符如中文）
fn is_version_boundary(c: char) -> bool {
    is_separator(c) || !c.is_ascii()
}

/// 将下划线分隔的版本号转换为点分隔
fn normalize_version(raw: &str) -> String {
    let trimmed = raw.trim_matches(|c: char| c == '_' || c == '.' || c == ' ');
    if trimmed.contains('_') && !trimmed.contains('.') {
        trimmed.replace('_', ".")
    } else {
        trimmed.to_string()
    }
}

/// 检查是否为有效的版本号格式（至少两段数字用点分隔）
fn is_valid_version(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}
