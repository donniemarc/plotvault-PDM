mod api;
mod convert;
mod db;
mod storage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    routing::get,
    Router,
};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

/// 检查文件名是否为 UUID 前缀格式（32 hex chars + '_' + original name）
fn is_uuid_prefix(name: &str) -> bool {
    if let Some((prefix, _rest)) = name.split_once('_') {
        prefix.len() == 32 && prefix.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    } else {
        false
    }
}

/// 同步状态
#[derive(Clone)]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub last_sync: Option<Instant>,
    pub last_sync_result: Option<String>,
}

impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            is_syncing: false,
            last_sync: None,
            last_sync_result: None,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    /// 图纸数据目录（library/ 真实目录 + blobs/ 版本归档）
    pub data_dir: PathBuf,
    /// 软件配置/缓存目录（dxf_cache/ + tmp/）
    pub config_dir: PathBuf,
    pub token: Option<String>,
    /// 文件同步状态（用于定时扫描 library/ 目录）
    pub sync_status: Arc<RwLock<SyncStatus>>,
}

/// 扫描 library/ 目录，将已存在的文件/文件夹导入数据库（幂等）。
/// 使用文件的相对路径（blob_path）进行去重，确保不会创建重复记录。
/// 返回值：(新增文件夹数, 新增文件数)
async fn scan_library_to_db(state: &AppState) -> (usize, usize) {
    let library_root = state.data_dir.join("library");
    if !library_root.exists() {
        return (0, 0);
    }
    let mut path_to_folder_id: HashMap<PathBuf, Option<i64>> = HashMap::new();
    path_to_folder_id.insert(library_root.clone(), None); // 根目录映射
    let new_folders = std::sync::atomic::AtomicUsize::new(0);
    let new_files = std::sync::atomic::AtomicUsize::new(0);

    // 递归扫描函数
    async fn scan_dir(
        dir: PathBuf,
        parent_folder_id: Option<i64>,
        state: &AppState,
        map: &mut HashMap<PathBuf, Option<i64>>,
        new_folders: &std::sync::atomic::AtomicUsize,
        new_files: &std::sync::atomic::AtomicUsize,
    ) {
        let entries = match tokio::task::spawn_blocking(move || {
            std::fs::read_dir(dir).map_err(|e| e.to_string())
        })
        .await
        {
            Ok(Ok(entries)) => entries,
            _ => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let folder_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                // 检查数据库是否已存在同名文件夹
                let existing =
                    db::find_folder_by_name(&state.db, parent_folder_id, folder_name).await;
                let folder_id = match existing {
                    Ok(Some(f)) => f.id,
                    _ => {
                        // 创建新文件夹
                        new_folders.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        db::create_folder(&state.db, folder_name, parent_folder_id)
                            .await
                            .unwrap_or(0)
                    }
                };
                map.insert(path.clone(), Some(folder_id));
                Box::pin(scan_dir(path, Some(folder_id), state, map, new_folders, new_files)).await;
            } else {
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                // 跳过 UUID 前缀文件（finalize_library_file 旧逻辑遗留的重复文件）
                if is_uuid_prefix(file_name) {
                    continue;
                }
                let ext = storage::ext_of(file_name);
                
                // 计算相对路径（从 library 根目录开始）
                let rel = match path.strip_prefix(&state.data_dir.join("library")) {
                    Ok(r) => format!("library/{}", r.to_string_lossy().replace('\\', "/")),
                    Err(_) => continue,
                };
                
                // 使用 blob_path 检查是否已存在（更可靠的去重机制）
                if let Ok(Some(_)) = db::find_version_by_blob_path(&state.db, &rel).await {
                    continue; // 已存在，跳过
                }
                
                // 获取文件大小（同步）
                let path_clone = path.clone();
                let size = tokio::task::spawn_blocking(move || {
                    std::fs::metadata(&path_clone)
                        .map(|m| m.len())
                        .unwrap_or(0)
                })
                .await
                .unwrap_or(0);
                // 计算 SHA256（同步）
                let path_clone2 = path.clone();
                let sha = tokio::task::spawn_blocking(move || compute_sha256(&path_clone2))
                    .await
                    .unwrap_or_default();
                // 创建文件记录
                if let Ok(file_id) =
                    db::create_file(&state.db, parent_folder_id, file_name, &ext, size as i64).await
                {
                    new_files.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let _ = db::insert_version(&state.db, file_id, 1, &rel, size as i64, &sha, "")
                        .await;
                }
            }
        }
        // 双向校验：数据库有记录但磁盘文件已不存在（用户在 NAS 共享盘直接移动/删除）→ 清理幽灵记录
        if let Ok(db_files) = db::list_files_by_folder(&state.db, parent_folder_id).await {
            for f in db_files {
                let Ok(Some(ver)) = db::get_version(&state.db, f.id, f.current_version).await else {
                    continue;
                };
                let abs = state.data_dir.join(&ver.blob_path);
                let exists = tokio::task::spawn_blocking(move || abs.exists())
                    .await
                    .unwrap_or(true);
                if !exists {
                    println!(
                        "sync: removing ghost record id={} name=\"{}\" (disk missing: {})",
                        f.id, f.name, ver.blob_path
                    );
                    if let Ok(paths) = db::delete_file(&state.db, f.id).await {
                        storage::remove_blobs(state, &paths);
                    }
                }
            }
        }
    }

    println!("scan: scanning library directory...");
    scan_dir(library_root, None, state, &mut path_to_folder_id, &new_folders, &new_files).await;
    let added_folders = new_folders.load(std::sync::atomic::Ordering::Relaxed);
    let added_files = new_files.load(std::sync::atomic::Ordering::Relaxed);
    println!("scan: added {added_folders} folders, {added_files} files");
    (added_folders, added_files)
}

/// 同步计算文件 SHA256
fn compute_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let bytes_read = match file.read(&mut buffer) {
            Ok(n) => n,
            Err(_) => break,
        };
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    hex::encode(hasher.finalize())
}

/// 启动迁移：把历史版本在 blobs 中的最新版本复制到 library/<文件夹路径>/ 真实目录，
/// 让 NAS 上能看到与 PlotVault PDM 一致的目录结构（失败的跳过，仍走 blobs 路径）。
async fn migrate_to_library(state: &AppState) {
    let folders = match db::list_folders(&state.db).await {
        Ok(f) => f,
        Err(_) => return,
    };
    // 先为所有文件夹建立真实目录骨架
    for folder in &folders {
        if let Ok(parts) = db::folder_path(&state.db, folder.id).await {
            let _ = storage::ensure_folder_dir(state, &parts);
        }
    }
    let files = match db::list_files(&state.db).await {
        Ok(f) => f,
        Err(_) => return,
    };
    let mut migrated = 0;
    let mut skipped = 0;
    for file in &files {
        let Ok(Some(ver)) = db::get_version(&state.db, file.id, file.current_version).await else {
            continue;
        };
        
        // 情况1：已经在library目录，检查文件是否存在
        if ver.blob_path.starts_with("library/") {
            let abs = state.data_dir.join(&ver.blob_path);
            if abs.exists() {
                // 文件存在，跳过
                skipped += 1;
                continue;
            } else {
                // 文件不存在，删除这条记录
                println!("migration: removing ghost record id={} name=\"{}\" (file missing)", file.id, file.name);
                if let Ok(paths) = db::delete_file(&state.db, file.id).await {
                    storage::remove_blobs(state, &paths);
                }
                continue;
            }
        }
        
        // 情况2：在blobs目录，需要迁移
        if !ver.blob_path.starts_with("blobs/") {
            continue;
        }
        let src_path = state.data_dir.join(&ver.blob_path);
        if !src_path.exists() {
            // 源文件不存在，检查library目录下是否有同名文件
            let parts = match file.folder_id {
                Some(fid) => db::folder_path(&state.db, fid).await.unwrap_or_default(),
                None => vec![],
            };
            let lib_path = storage::folder_dir(state, &parts).join(storage::safe_name(&file.name));
            if lib_path.exists() {
                // library目录下已有文件，直接更新数据库
                if let Ok(rel) = storage::rel_of_public(state, &lib_path) {
                    let _ = db::update_version_blob_path(&state.db, file.id, file.current_version, &rel).await;
                    println!("migration: updated record id={} to library path", file.id);
                }
            } else {
                // 两边都没有文件，删除记录
                println!("migration: removing orphan record id={} name=\"{}\"", file.id, file.name);
                if let Ok(paths) = db::delete_file(&state.db, file.id).await {
                    storage::remove_blobs(state, &paths);
                }
            }
            continue;
        }
        let parts = match file.folder_id {
            Some(fid) => db::folder_path(&state.db, fid).await.unwrap_or_default(),
            None => vec![],
        };
        match storage::copy_to_library(state, &ver.blob_path, &parts, &file.name) {
            Ok(rel) => {
                if db::update_version_blob_path(&state.db, file.id, file.current_version, &rel).await.is_ok() {
                    let _ = std::fs::remove_file(state.data_dir.join(&ver.blob_path));
                    if let Some(dir) = ver.blob_path.rsplit_once('/').map(|(d, _)| d.to_string()) {
                        let _ = std::fs::remove_dir(state.data_dir.join(dir));
                    }
                    migrated += 1;
                }
            }
            Err(_) => continue,
        }
    }
    println!("migration: migrated {migrated} files, skipped {skipped}, total {total}", total = files.len());
}

/// 启动清理：扫描 library 目录，删除 UUID 副本文件（finalize_library_file 旧逻辑遗留产物）。
/// 如果 `library/<dir>/<uuid>_<name>.ext` 与 `library/<dir>/<name>.ext` 同时存在，
/// 则 UUID 副本为重复文件，删除其磁盘文件并清理数据库记录。
async fn cleanup_duplicates(state: &AppState) {
    let library_root = state.data_dir.join("library");
    if !library_root.exists() {
        return;
    }

    // 收集需要清理的 UUID 文件信息（路径 + 文件名）
    struct DupEntry {
        path: std::path::PathBuf,
        file_name: String,
    }
    let mut dups: Vec<DupEntry> = Vec::new();

    fn scan_dir(
        dir: &std::path::Path,
        dups: &mut Vec<DupEntry>,
    ) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, dups);
                continue;
            }
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !is_uuid_prefix(&file_name) {
                continue;
            }
            if let Some((_uuid, original)) = file_name.split_once('_') {
                let original_path = dir.join(original);
                if original_path.exists() {
                    dups.push(DupEntry { path, file_name });
                }
            }
        }
    }

    scan_dir(&library_root, &mut dups);
    if dups.is_empty() {
        return;
    }

    let mut removed = 0u32;
    for dup in &dups {
        // 删除磁盘文件
        if std::fs::remove_file(&dup.path).is_ok() {
            removed += 1;
            println!("cleanup: removed UUID duplicate: {}", dup.path.display());
        }
        // 删除数据库记录：查找名为 dup.file_name 的文件记录并删除
        if let Ok(Some(file)) = db::find_file_by_name_exact(&state.db, &dup.file_name).await {
            if let Ok(paths) = db::delete_file(&state.db, file.id).await {
                storage::remove_blobs(state, &paths);
                println!("cleanup: removed db record id={} name=\"{}\"", file.id, file.name);
            }
        }
    }
    if removed > 0 {
        println!("cleanup: removed {removed} UUID duplicate files from library");
    }
}

/// 等待数据库就绪：compose 里 db 容器健康前服务端可能先起，
/// 这里最多重试 max_attempts 次（每次间隔 2s），DB 起来后自动连上。
async fn connect_with_retry(database_url: &str, max_attempts: u32) -> Result<sqlx::PgPool> {
    let mut last_err: Option<sqlx::Error> = None;
    for attempt in 1..=max_attempts {
        match sqlx::PgPool::connect(database_url).await {
            Ok(pool) => {
                println!("database: connected to PostgreSQL (attempt {attempt}/{max_attempts})");
                return Ok(pool);
            }
            Err(e) => {
                last_err = Some(e);
                if attempt == 1 || attempt % 10 == 0 || attempt == max_attempts {
                    println!("database: connection attempt {attempt}/{max_attempts} failed, retrying in 2s...");
                }
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
    Err(anyhow::anyhow!(
        "could not connect to PostgreSQL after {max_attempts} attempts: {last_err:?}"
    ))
}

#[tokio::main]
async fn main() -> Result<()> {
    // DATA_DIR=图纸数据（library 真实目录 + blobs 版本归档）；CONFIG_DIR=软件配置/缓存（dxf_cache + tmp）
    let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
    let config_dir = PathBuf::from(std::env::var("CONFIG_DIR").unwrap_or_else(|_| "./config".into()));
    std::fs::create_dir_all(data_dir.join("blobs"))?;
    std::fs::create_dir_all(data_dir.join("library"))?;
    std::fs::create_dir_all(config_dir.join("dxf_cache"))?;
    std::fs::create_dir_all(config_dir.join("tmp"))?;

    // 数据库改为 PostgreSQL（异步连接池）。DATABASE_URL 示例：
    //   postgres://plotvault_pdm:plotvault_pdm@db:5432/plotvault_pdm   （Docker Compose 内）
    //   postgres://postgres:postgres@localhost:5432/plotvault_pdm （本机裸跑）
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable must be set");
    // 最多等 240s（120×2s），DB 容器初始化慢时服务端自动等待而不是立刻退出
    let pool = connect_with_retry(&database_url, 120).await?;
    db::init(&pool).await?;
    println!("database: tables ready");

    let token = std::env::var("API_TOKEN").ok().filter(|s| !s.is_empty());
    if let Some(t) = &token {
        println!("API_TOKEN configured: access requires Authorization: Bearer {}", t);
    } else {
        println!("WARNING: API_TOKEN not set, API is open to the LAN");
    }

    let state = AppState {
        db: pool,
        data_dir,
        config_dir,
        token,
        sync_status: Arc::new(RwLock::new(SyncStatus::default())),
    };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let app = Router::new()
        .route("/", get(|| async { "plotvault-pdm server is running" }))
        .nest("/api", api::routes(state.clone()))
        // axum 默认 body limit 仅 2MB，图纸文件（DWG/STEP）可达数百 MB，需放开。
        // 大文件仍按流式写入磁盘（storage::stream_field_to_temp），此处只放宽上限。
        .layer(DefaultBodyLimit::max(4 * 1024 * 1024 * 1024))
        .layer(cors);

    let addr = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8642".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("plotvault-pdm server listening on http://{addr}");

    // 后台异步执行扫描和迁移，服务立即可用
    let state_bg = state.clone();
    tokio::spawn(async move {
        // 首次启动立即执行一次同步
        {
            let mut status = state_bg.sync_status.write().await;
            status.is_syncing = true;
        }
        // 先清理 library 中的 UUID 重复文件（旧 finalize_library_file 逻辑遗留）
        cleanup_duplicates(&state_bg).await;
        let (folders, files) = scan_library_to_db(&state_bg).await;
        migrate_to_library(&state_bg).await;
        {
            let mut status = state_bg.sync_status.write().await;
            status.is_syncing = false;
            status.last_sync = Some(Instant::now());
            status.last_sync_result = Some(format!("新增 {} 个文件夹，共 {} 个文件", folders, files));
        }
        println!("scan: initial sync completed");

        // 定时同步任务：每 30 秒扫描一次 library/ 目录
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.tick().await; // 跳过第一次立即触发的 tick
        loop {
            interval.tick().await;
            {
                let mut status = state_bg.sync_status.write().await;
                status.is_syncing = true;
            }
            let (folders, files) = scan_library_to_db(&state_bg).await;
            {
                let mut status = state_bg.sync_status.write().await;
                status.is_syncing = false;
                status.last_sync = Some(Instant::now());
                status.last_sync_result = Some(format!("新增 {} 个文件夹，共 {} 个文件", folders, files));
            }
        }
    });

    axum::serve(listener, app).await?;
    Ok(())
}
