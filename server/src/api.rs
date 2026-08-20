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

pub type ApiResult<T> = Result<T, AppError>;

async fn auth(State(state): State<AppState>, req: Request, next: Next) -> Result<Response, AppError> {
    if let Some(token) = &state.token {
        let ok = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.trim().to_string())
            == Some(token.clone());
        if !ok {
            return Err(AppError { status: StatusCode::UNAUTHORIZED, msg: "unauthorized".into() });
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
        .route("/files", post(upload_file))
        .route("/files/{id}", patch(patch_file).delete(delete_file))
        .route("/files/{id}/versions", get(list_versions).post(add_version))
        .route("/files/{id}/download", get(download))
        .route("/files/{id}/preview", get(preview))
        .route("/files/{id}/dxf", get(get_dxf))
        .route("/search", get(search))
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "plotvault-pdm" }))
}

async fn tree(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let folders = db::list_folders(&state.db).await?;
    let files = db::list_files(&state.db).await?;
    Ok(Json(json!({ "folders": folders, "files": files })))
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
    let folder = db::get_folder(&state.db, id).await?.ok_or_else(|| AppError::not_found("folder not found"))?;
    Ok(Json(json!(folder)))
}

async fn delete_folder(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let parts = db::folder_path(&state.db, id).await?;
    let file_ids = db::file_ids_under_folder(&state.db, id).await?;
    for fid in file_ids {
        storage::remove_file_blobs(&state, fid);
    }
    db::delete_folder(&state.db, id).await?;
    // 同步删除 NAS 真实目录（含子目录与最新版本文件）
    storage::remove_folder_dir(&state, &parts);
    Ok(StatusCode::NO_CONTENT)
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
}

async fn patch_file(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<FilePatch>,
) -> ApiResult<Json<Value>> {
    let name = body.name.as_deref().map(|s| s.trim());
    if let Some(n) = name {
        if n.is_empty() {
            return Err(AppError::bad("filename cannot be empty"));
        }
    }
    let desc = body.description.as_deref().map(|s| s.trim());
    db::patch_file(&state.db, id, name, body.folder_id, desc).await?;
    let file = db::get_file(&state.db, id).await?.ok_or_else(|| AppError::not_found("file not found"))?;
    // 同步真实文件：重命名/移动时把 library 中的当前版本文件移动到新位置
    let cur_ver = db::get_version(&state.db, id, file.current_version).await?;
    if let Some(cv) = cur_ver {
        if cv.blob_path.starts_with("library/") {
            let folder_parts = match file.folder_id {
                Some(fid) => db::folder_path(&state.db, fid).await?,
                None => vec![],
            };
            let rel = storage::move_library_file(&state, &cv.blob_path, &folder_parts, &file.name)?;
            if rel != cv.blob_path {
                db::update_version_blob_path(&state.db, id, file.current_version, &rel).await?;
            }
        }
    }
    Ok(Json(json!(file)))
}

async fn delete_file(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let paths = db::delete_file(&state.db, id).await?;
    // 同时删除 library 中的当前版本真实文件与 blobs 归档（安全删除，不误删同目录其它文件）
    storage::remove_blobs(&state, &paths);
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

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> ApiResult<Json<Value>> {
    let files = db::search_files(&state.db, q.q.trim()).await?;
    Ok(Json(json!({ "files": files })))
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