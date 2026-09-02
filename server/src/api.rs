use std::path::PathBuf;

use axum::{
    body::Body,
    extract::{multipart::Multipart, Path, Query, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, patch, post},
    Router,
};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use tokio_util::io::ReaderStream;

use crate::{convert, db, storage, AppState};

pub struct AppError {
    pub status: StatusCode,
    pub msg: String,
}

impl AppError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, msg: msg.into() }
    }
    pub fn bad(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, msg: msg.into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, msg: msg.into() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.msg }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<axum::Error> for AppError {
    fn from(e: axum::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(e: axum::extract::multipart::MultipartError) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::internal(e.to_string())
    }
}

pub type ApiResult<T> = Result<T, AppError>;

async fn auth(State(state): State<AppState>, req: Request, next: Next) -> Result<Response, AppError> {
    if let Some(token) = &state.token {
        let provided_token = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.trim());
        
        match provided_token {
            Some(provided) => {
                let token_bytes = token.as_bytes();
                let provided_bytes = provided.as_bytes();
                let len_match = token_bytes.len() == provided_bytes.len();
                let content_match: bool = token_bytes.ct_eq(provided_bytes).into();
                if !len_match || !content_match {
                    return Err(AppError { status: StatusCode::UNAUTHORIZED, msg: "unauthorized".into() });
                }
            }
            None => {
                return Err(AppError { status: StatusCode::UNAUTHORIZED, msg: "unauthorized".into() });
            }
        }
    }
    Ok(next.run(req).await)
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tree", get(tree))
        .route("/folders", post(create_folder))
        .route("/folders/{id}", patch(patch_folder).delete(delete_folder))
        .route("/folders/{id}/props", patch(update_folder_props))
        .route("/files", post(upload_file))
        .route("/files/{id}", patch(patch_file).delete(delete_file))
        .route("/files/{id}/versions", get(list_versions).post(add_version))
        .route("/files/{id}/download", get(download))
        .route("/files/{id}/preview", get(preview))
        .route("/files/{id}/dxf", get(get_dxf))
        .route("/files/{id}/archive-list", get(archive_list))
        .route("/files/{id}/archive-entry", get(archive_entry))
        .route("/files/{id}/disk-path", get(disk_path))
        .route("/folders/{id}/disk-path", get(folder_disk_path))
        .route("/search", get(search))
        .route("/sync/status", get(sync_status))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "plotvault-pdm" }))
}

#[derive(Deserialize)]
struct TreeQuery {
    parent_id: Option<i64>,
    page: Option<i64>,
    limit: Option<i64>,
}

async fn tree(
    State(state): State<AppState>,
    Query(q): Query<TreeQuery>,
) -> ApiResult<Json<Value>> {
    let page = q.page.unwrap_or(1).max(1);
    let limit = q.limit.unwrap_or(1000).min(5000).max(1);
    
    let folders = if let Some(parent_id) = q.parent_id {
        db::list_folders_by_parent(&state.db, parent_id).await?
    } else {
        db::list_folders(&state.db).await?
    };
    
    let files = if let Some(parent_id) = q.parent_id {
        db::list_files_by_folder(&state.db, Some(parent_id)).await?
    } else {
        db::list_files(&state.db).await?
    };
    
    Ok(Json(json!({ 
        "folders": folders, 
        "files": files,
        "page": page,
        "limit": limit,
        "total_folders": folders.len(),
        "total_files": files.len()
    })))
}

// ---------- folders ----------

#[derive(Deserialize)]
struct FolderCreate {
    name: String,
    #[serde(default)]
    parent_id: Option<i64>,
}

async fn create_folder(
    State(state): State<AppState>,
    Json(body): Json<FolderCreate>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::bad("folder name is required"));
    }
    if db::find_folder_by_name(&state.db, body.parent_id, name).await?.is_some() {
        return Err(AppError::bad("同层级下已存在同名文件夹"));
    }
    let id = db::create_folder(&state.db, name, body.parent_id).await?;
    let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::internal("folder create failed"))?;
    // 同步在 NAS 创建真实目录
    let parts = db::folder_path(&state.db, id).await?;
    storage::ensure_folder_dir(&state, &parts)?;
    Ok((StatusCode::CREATED, Json(json!(folder))))
}

#[derive(Deserialize)]
struct FolderPatch {
    name: Option<String>,
    #[serde(default)]
    parent_id: Option<i64>,
}

async fn patch_folder(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<FolderPatch>,
) -> ApiResult<Json<Value>> {
    if let Some(name) = body.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::bad("folder name is required"));
        }
        // 重名检测（同父级）
        let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
        if let Some(existing) = db::find_folder_by_name(&state.db, folder.parent_id, name).await? {
            if existing.id != id {
                return Err(AppError::bad("同层级下已存在同名文件夹"));
            }
        }
        let old_parts = db::folder_path(&state.db, id).await?;
        db::rename_folder(&state.db, id, name).await?;
        // 同步重命名 NAS 真实目录
        let new_parts = db::folder_path(&state.db, id).await?;
        storage::rename_folder_dir(&state, &old_parts, &new_parts)?;
    }

    // 移动文件夹（改变父级）：0 = 根目录（parent_id = NULL）
    if let Some(raw_parent_id) = body.parent_id {
        let new_parent_id: Option<i64> = if raw_parent_id == 0 { None } else { Some(raw_parent_id) };
        let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
        // 不能移动到自己
        if new_parent_id == Some(id) {
            return Err(AppError::bad("不能将文件夹移动到自身"));
        }
        // 循环检测：不能移动到自己的子孙目录
        if let Some(pid) = new_parent_id {
            if db::is_descendant(&state.db, id, pid).await? {
                return Err(AppError::bad("不能将文件夹移动到其子目录下"));
            }
        }
        // 同父级则视为无效
        if folder.parent_id == new_parent_id {
            let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
            return Ok(Json(json!(folder)));
        }
        // 重名检测（新父级下）
        if let Some(existing) = db::find_folder_by_name(&state.db, new_parent_id, &folder.name).await? {
            if existing.id != id {
                return Err(AppError::bad("目标位置已存在同名文件夹"));
            }
        }
        let old_parts = db::folder_path(&state.db, id).await?;
        db::move_folder(&state.db, id, new_parent_id).await?;
        // 同步移动 NAS 真实目录
        let new_parts = db::folder_path(&state.db, id).await?;
        storage::rename_folder_dir(&state, &old_parts, &new_parts)?;
    }

    let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
    Ok(Json(json!(folder)))
}

async fn delete_folder(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let parts = db::folder_path(&state.db, id).await?;
    let file_ids = db::file_ids_under_folder(&state.db, id).await?;
    // 安全删除每个文件的 blob（跳过仍被其它文件引用的共享 blob）
    for fid in &file_ids {
        let versions = db::list_versions(&state.db, *fid).await.unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for v in &versions {
            if seen.insert(v.blob_path.as_str()) {
                if !db::is_blob_referenced_by_others(&state.db, &v.blob_path, *fid).await.unwrap_or(false) {
                    storage::remove_blobs(&state, &[v.blob_path.clone()]);
                }
            }
        }
    }
    db::delete_folder(&state.db, id).await?;
    // 同步删除 NAS 真实目录（含子目录与最新版本文件）
    storage::remove_folder_dir(&state, &parts);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct FolderProps {
    name: String,
    code: Option<String>,
    stage: Option<String>,
    status: Option<String>,
    description: Option<String>,
    remarks: Option<String>,
    creator: Option<String>,
}

async fn update_folder_props(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<FolderProps>,
) -> ApiResult<Json<Value>> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::bad("folder name is required"));
    }
    // 重名检测（同父级）
    let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
    if let Some(existing) = db::find_folder_by_name(&state.db, folder.parent_id, name).await? {
        if existing.id != id {
            return Err(AppError::bad("同层级下已存在同名文件夹"));
        }
    }
    
    let code = body.code.as_deref().unwrap_or("");
    let stage = body.stage.as_deref().unwrap_or("");
    let status = body.status.as_deref().unwrap_or("");
    let description = body.description.as_deref().unwrap_or("");
    let remarks = body.remarks.as_deref().unwrap_or("");
    let creator = body.creator.as_deref().unwrap_or("");
    
    db::update_folder_props(&state.db, id, name, code, stage, status, description, remarks, creator).await?;
    
    // 如果名称变更，需要同步重命名NAS真实目录
    if name != folder.name {
        let old_parts = db::folder_path(&state.db, id).await?;
        // 先更新名称
        db::rename_folder(&state.db, id, name).await?;
        let new_parts = db::folder_path(&state.db, id).await?;
        storage::rename_folder_dir(&state, &old_parts, &new_parts)?;
    }
    
    let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
    Ok(Json(json!(folder)))
}

// ---------- files ----------

#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    new_file: bool,
}

async fn upload_file(
    State(state): State<AppState>,
    Query(q): Query<UploadQuery>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let mut filename: Option<String> = None;
    let mut folder_id: Option<i64> = None;
    let mut comment = String::new();
    let mut saved: Option<(PathBuf, String, u64)> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("").to_string().as_str() {
            "folder_id" => {
                folder_id = field.text().await.ok().and_then(|s| s.trim().parse::<i64>().ok());
            }
            "comment" => comment = field.text().await.unwrap_or_default(),
            "file" => {
                if field.file_name().is_some() {
                    filename = field.file_name().map(|s| s.to_string());
                    saved = Some(storage::stream_field_to_temp(&state, field).await?);
                }
            }
            _ => {}
        }
    }

    let filename = filename.ok_or_else(|| AppError::bad("no file part in upload"))?;
    let clean_name = filename.trim().to_string();
    if clean_name.is_empty() {
        return Err(AppError::bad("empty filename"));
    }
    let (tmp, sha, size) = saved.ok_or_else(|| AppError::bad("no file data received"))?;
    let ext = storage::ext_of(&clean_name);

    let result: Result<Value, AppError> = async {
        let existing = if q.new_file {
            None
        } else {
            db::find_file_by_name(&state.db, folder_id, &clean_name).await?
        };

        if let Some(existing_file) = existing {
            // SHA256 去重：如果文件内容与某个已有版本完全相同，跳过上传
            if let Some(dup_ver) = db::find_version_by_sha256(&state.db, existing_file.id, &sha).await? {
                let _ = std::fs::remove_file(&tmp);
                let file = db::get_file(&state.db, existing_file.id).await?.unwrap();
                return Ok(json!({
                    "created": "version",
                    "file": file,
                    "version_id": dup_ver.id,
                    "version_no": dup_ver.version_no,
                    "dedup": true
                }));
            }
            let folder_parts = match existing_file.folder_id {
                Some(fid) => db::folder_path(&state.db, fid).await?,
                None => vec![],
            };
            let version_no = db::next_version_no(&state.db, existing_file.id).await?;
            // 归档旧当前版本（library 下）到 blobs，保留历史
            let cur_ver = db::get_version(&state.db, existing_file.id, existing_file.current_version).await?;
            if let Some(cv) = cur_ver {
                if cv.blob_path.starts_with("library/") {
                    let archive_rel =
                        storage::archive_blob(&state, existing_file.id, existing_file.current_version, &existing_file.ext, &cv.blob_path)?;
                    db::update_version_blob_path(&state.db, existing_file.id, existing_file.current_version, &archive_rel).await?;
                }
            }
            let rel = storage::finalize_library_file(&state, &folder_parts, &clean_name, &tmp)?;
            let vid = db::insert_version(&state.db, existing_file.id, version_no, &rel, size as i64, &sha, &comment).await?;
            db::update_file_size(&state.db, existing_file.id, size as i64, version_no).await?;
            let file = db::get_file(&state.db, existing_file.id).await?.unwrap();
            Ok(json!({ "created": "version", "file": file, "version_id": vid, "version_no": version_no }))
        } else {
            let folder_parts = match folder_id {
                Some(fid) => db::folder_path(&state.db, fid).await?,
                None => vec![],
            };
            let file_id = db::create_file(&state.db, folder_id, &clean_name, &ext, size as i64).await?;
            let version_no = 1i64;
            let rel = storage::finalize_library_file(&state, &folder_parts, &clean_name, &tmp)?;
            let vid = db::insert_version(&state.db, file_id, version_no, &rel, size as i64, &sha, &comment).await?;
            let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::internal("file create failed"))?;
            Ok(json!({ "created": "file", "file": file, "version_id": vid, "version_no": version_no }))
        }
    }
    .await;

    match result {
        Ok(v) => Ok((StatusCode::CREATED, Json(v))),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

async fn add_version(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let mut comment = String::new();
    let mut saved: Option<(PathBuf, String, u64)> = None;

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("").to_string().as_str() {
            "comment" => comment = field.text().await.unwrap_or_default(),
            "file" => {
                if field.file_name().is_some() {
                    saved = Some(storage::stream_field_to_temp(&state, field).await?);
                }
            }
            _ => {}
        }
    }

    let (tmp, sha, size) = saved.ok_or_else(|| AppError::bad("no file data received"))?;

    let result: Result<Value, AppError> = async {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        // SHA256 去重：如果文件内容与某个已有版本完全相同，复用已有版本
        if let Some(dup_ver) = db::find_version_by_sha256(&state.db, file_id, &sha).await? {
            let _ = std::fs::remove_file(&tmp);
            return Ok(json!({
                "version_id": dup_ver.id,
                "version_no": dup_ver.version_no,
                "dedup": true
            }));
        }
        let folder_parts = match file.folder_id {
            Some(fid) => db::folder_path(&state.db, fid).await?,
            None => vec![],
        };
        let version_no = db::next_version_no(&state.db, file_id).await?;
        // 归档旧当前版本到 blobs，保留历史
        let cur_ver = db::get_version(&state.db, file_id, file.current_version).await?;
        if let Some(cv) = cur_ver {
            if cv.blob_path.starts_with("library/") {
                let archive_rel = storage::archive_blob(&state, file_id, file.current_version, &file.ext, &cv.blob_path)?;
                db::update_version_blob_path(&state.db, file_id, file.current_version, &archive_rel).await?;
            }
        }
        let rel = storage::finalize_library_file(&state, &folder_parts, &file.name, &tmp)?;
        let vid = db::insert_version(&state.db, file_id, version_no, &rel, size as i64, &sha, &comment).await?;
        db::update_file_size(&state.db, file_id, size as i64, version_no).await?;
        Ok(json!({ "version_id": vid, "version_no": version_no }))
    }
    .await;

    match result {
        Ok(v) => Ok((StatusCode::CREATED, Json(v))),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

async fn list_versions(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    let versions = db::list_versions(&state.db, file_id).await?;
    Ok(Json(json!({ "file": file, "versions": versions })))
}

#[derive(Deserialize)]
struct VersionQuery {
    version: Option<i64>,
}

#[derive(Deserialize)]
struct FilePatch {
    name: Option<String>,
    #[serde(default)]
    folder_id: Option<Option<i64>>,
    description: Option<String>,
    code: Option<String>,
    stage: Option<String>,
    status: Option<String>,
    remarks: Option<String>,
    creator: Option<String>,
    drawing_size: Option<String>,
    source_file_type: Option<String>,
    source_file_version: Option<String>,
    other_info: Option<String>,
    publish_time: Option<String>,
}

async fn patch_file(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<FilePatch>,
) -> ApiResult<Json<Value>> {
    let cur_file = db::get_file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    let name = body.name.as_deref().map(|s| s.trim());
    if let Some(n) = name {
        if n.is_empty() {
            return Err(AppError::bad("filename cannot be empty"));
        }
    }
    // 重名检测：确定目标文件夹和目标文件名后检查同级是否已存在同名文件
    let target_folder = body.folder_id.as_ref().unwrap_or(&cur_file.folder_id);
    let target_name = name.unwrap_or(&cur_file.name);
    if let Some(existing) = db::find_file_by_name(&state.db, *target_folder, target_name).await? {
        if existing.id != id {
            return Err(AppError::bad("目标位置已存在同名文件"));
        }
    }

    // 同步真实文件：先移动磁盘文件（重命名/移动），成功后再更新数据库。
    // 顺序保证：若移动失败（如大文件跨卷 copy 中断、源被并发删除），数据库不变，
    // 文件记录仍指向原位置，不会出现"数据库已指向新位置但磁盘文件不存在"的半成品状态。
    let mut new_rel: Option<String> = None;
    if let Some(cv) = db::get_version(&state.db, id, cur_file.current_version).await? {
        if cv.blob_path.starts_with("library/") {
            let folder_parts = match *target_folder {
                Some(fid) => db::folder_path(&state.db, fid).await?,
                None => vec![],
            };
            let rel = storage::move_library_file(&state, &cv.blob_path, &folder_parts, target_name)?;
            if rel != cv.blob_path {
                new_rel = Some(rel);
            }
        }
    }

    // 更新所有属性字段
    let new_name = name.unwrap_or(&cur_file.name);
    let new_code = body.code.as_deref().unwrap_or(&cur_file.code);
    let new_stage = body.stage.as_deref().unwrap_or(&cur_file.stage);
    let new_status = body.status.as_deref().unwrap_or(&cur_file.status);
    let new_desc = body.description.as_deref().unwrap_or(&cur_file.description);
    let new_remarks = body.remarks.as_deref().unwrap_or(&cur_file.remarks);
    let new_creator = body.creator.as_deref().unwrap_or(&cur_file.creator);
    let new_drawing_size = body.drawing_size.as_deref().unwrap_or(&cur_file.drawing_size);
    let new_source_file_type = body.source_file_type.as_deref().unwrap_or(&cur_file.source_file_type);
    let new_source_file_version = body.source_file_version.as_deref().unwrap_or(&cur_file.source_file_version);
    let new_other_info = body.other_info.as_deref().unwrap_or(&cur_file.other_info);
    let new_publish_time = body.publish_time.as_deref().unwrap_or(&cur_file.publish_time);
    
    db::update_file_props(&state.db, id, new_name, new_code, new_stage, new_status, new_desc, new_remarks, new_creator, new_drawing_size, new_source_file_type, new_source_file_version, new_other_info, new_publish_time).await?;
    
    // 如果有文件夹或名称变更，还需要更新folder_id
    if let Some(fid) = body.folder_id {
        db::patch_file_folder(&state.db, id, fid).await?;
    }
    
    if let Some(rel) = new_rel {
        db::update_version_blob_path(&state.db, id, cur_file.current_version, &rel).await?;
    }
    let file = db::get_file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    Ok(Json(json!(file)))
}

async fn delete_file(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let paths = db::delete_file(&state.db, id).await?;
    // 安全删除 blob：跳过仍被其它文件引用的共享 blob
    let unique: std::collections::HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();
    for rel in &unique {
        if db::is_blob_referenced_by_others(&state.db, rel, id).await.unwrap_or(false) {
            continue;
        }
        storage::remove_blobs(&state, &[rel.to_string()]);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn download(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Response> {
    let (file, rel_path) = {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        let version = match q.version {
            Some(v) => db::get_version(&state.db, file_id, v).await?.ok_or_else(|| AppError::not_found("version not found"))?,
            None => db::get_version(&state.db, file_id, file.current_version).await?
                .ok_or_else(|| AppError::not_found("current version missing"))?,
        };
        (file, version.blob_path)
    };
    let path = storage::blob_abs_path(&state, &rel_path);
    serve_file(path, Some(&file.name), true).await
}

async fn preview(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Response> {
    let (file, rel_path, sha) = {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        let version = match q.version {
            Some(v) => db::get_version(&state.db, file_id, v).await?.ok_or_else(|| AppError::not_found("version not found"))?,
            None => db::get_version(&state.db, file_id, file.current_version).await?
                .ok_or_else(|| AppError::not_found("current version missing"))?,
        };
        (file, version.blob_path, version.sha256)
    };

    if storage::is_dwg(&file.ext) {
        let dxf_path = convert::dwg_to_dxf(&state, &rel_path, &sha).await?;
        let stem = file.name.trim_end_matches(&format!(".{}", file.ext));
        let dxf_name = format!("{}.dxf", stem);
        serve_file(dxf_path, Some(&dxf_name), false).await
    } else {
        let path = storage::blob_abs_path(&state, &rel_path);
        serve_file(path, Some(&file.name), false).await
    }
}

async fn get_dxf(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Response> {
    let (file, rel_path, sha) = {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        if !storage::is_dwg(&file.ext) {
            return Err(AppError::bad("not a DWG file"));
        }
        let version = match q.version {
            Some(v) => db::get_version(&state.db, file_id, v).await?.ok_or_else(|| AppError::not_found("version not found"))?,
            None => db::get_version(&state.db, file_id, file.current_version).await?
                .ok_or_else(|| AppError::not_found("current version missing"))?,
        };
        (file, version.blob_path, version.sha256)
    };

    let dxf_path = convert::dwg_to_dxf(&state, &rel_path, &sha).await?;
    let stem = file.name.trim_end_matches(&format!(".{}", file.ext));
    let dxf_name = format!("{}.dxf", stem);
    serve_file(dxf_path, Some(&dxf_name), true).await
}

async fn archive_list(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(q): Query<VersionQuery>,
) -> ApiResult<Json<Value>> {
    let (file, rel_path) = {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        let ext = file.ext.to_lowercase();
        if ext != "zip" && ext != "rar" {
            return Err(AppError::bad("not a ZIP or RAR file"));
        }
        let version = match q.version {
            Some(v) => db::get_version(&state.db, file_id, v).await?.ok_or_else(|| AppError::not_found("version not found"))?,
            None => db::get_version(&state.db, file_id, file.current_version).await?
                .ok_or_else(|| AppError::not_found("current version missing"))?,
        };
        (file, version.blob_path)
    };

    let path = storage::blob_abs_path(&state, &rel_path);
    let entries = match file.ext.to_lowercase().as_str() {
        "zip" => tokio::task::spawn_blocking(move || convert::list_zip_entries(&path)).await??,
        "rar" => convert::list_rar_entries(&path).await?,
        _ => return Err(AppError::bad("unsupported archive format")),
    };

    Ok(Json(json!({ "entries": entries })))
}

#[derive(Deserialize)]
struct ArchiveEntryQuery {
    version: Option<i64>,
    path: String,
}

async fn archive_entry(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
    Query(q): Query<ArchiveEntryQuery>,
) -> ApiResult<Response> {
    let (file, rel_path) = {
        let file = db::get_file(&state.db, file_id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
        let ext = file.ext.to_lowercase();
        if ext != "zip" && ext != "rar" {
            return Err(AppError::bad("not a ZIP or RAR file"));
        }
        let version = match q.version {
            Some(v) => db::get_version(&state.db, file_id, v).await?.ok_or_else(|| AppError::not_found("version not found"))?,
            None => db::get_version(&state.db, file_id, file.current_version).await?
                .ok_or_else(|| AppError::not_found("current version missing"))?,
        };
        (file, version.blob_path)
    };

    let archive_path = storage::blob_abs_path(&state, &rel_path);
    let entry_name = q.path;

    let stem = file.name.trim_end_matches(&format!(".{}", file.ext));
    let tmp_name = format!("{}_{}_{}", stem, entry_name.replace('/', "_"), uuid::Uuid::new_v4());
    let tmp_path = state.config_dir.join("tmp").join(&tmp_name);

    // ensure tmp dir exists
    let _ = tokio::fs::create_dir_all(state.config_dir.join("tmp")).await;

    match file.ext.to_lowercase().as_str() {
        "zip" => {
            let ap = archive_path.clone();
            let ep = entry_name.clone();
            let tp = tmp_path.clone();
            tokio::task::spawn_blocking(move || convert::extract_zip_entry(&ap, &ep, &tp)).await??;
        }
        "rar" => {
            convert::extract_rar_entry(&archive_path, &entry_name, &tmp_path).await?;
        }
        _ => return Err(AppError::bad("unsupported archive format")),
    }

    let display_name = format!("{}/{}", stem, entry_name);
    serve_file(tmp_path, Some(&display_name), false).await
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let files = db::search_files(&state.db, q.q.trim()).await?;
    Ok(Json(json!({ "files": files })))
}

/// 返回文件所在文件夹在磁盘上的绝对路径（用于客户端打开文件夹）
async fn disk_path(
    State(state): State<AppState>,
    Path(file_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let file = db::get_file(&state.db, file_id).await?
        .ok_or_else(|| AppError::not_found("file not found"))?;
    // 使用文件夹路径，而不是文件路径（文件可能不存在）
    let folder_parts = match file.folder_id {
        Some(fid) => db::folder_path(&state.db, fid).await.unwrap_or_default(),
        None => vec![],
    };
    let folder_abs = storage::folder_dir(&state, &folder_parts);
    Ok(Json(json!({
        "path": folder_abs.to_string_lossy(),
        "name": file.name,
    })))
}

/// 返回文件夹在磁盘上的绝对路径（用于客户端打开文件夹）
async fn folder_disk_path(
    State(state): State<AppState>,
    Path(folder_id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let folder = db::get_folder(&state.db, folder_id).await?
        .ok_or_else(|| AppError::not_found("folder not found"))?;
    let folder_parts = db::folder_path(&state.db, folder_id).await.unwrap_or_default();
    let folder_abs = storage::folder_dir(&state, &folder_parts);
    Ok(Json(json!({
        "path": folder_abs.to_string_lossy(),
        "name": folder.name,
    })))
}

async fn serve_file(path: PathBuf, name: Option<&str>, attachment: bool) -> ApiResult<Response> {
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::not_found("blob file missing"))?;
    let meta = file.metadata().await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let ext = name.map(storage::ext_of).unwrap_or_default();
    let mime = storage::mime_for(&ext);

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CONTENT_LENGTH, meta.len().to_string());

    if let Some(name) = name {
        let enc = percent_encode(name.as_bytes(), NON_ALPHANUMERIC).to_string();
        let kind = if attachment { "attachment" } else { "inline" };
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!("{}; filename*=UTF-8''{}", kind, enc),
        );
    }

    builder.body(body).map_err(|e| AppError::internal(e.to_string()))
}

/// 获取同步状态
async fn sync_status(State(state): State<AppState>) -> Json<Value> {
    let status = state.sync_status.read().await;
    let last_sync_secs = status.last_sync.map(|t| t.elapsed().as_secs());
    Json(json!({
        "is_syncing": status.is_syncing,
        "last_sync_secs": last_sync_secs,
        "last_sync_result": status.last_sync_result,
    }))
}