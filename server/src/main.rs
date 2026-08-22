mod api;
mod convert;
mod db;
mod storage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    routing::get,
    Router,
};
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    /// 图纸数据目录（library/ 真实目录 + blobs/ 版本归档）
    pub data_dir: PathBuf,
    /// 软件配置/缓存目录（dxf_cache/ + tmp/）
    pub config_dir: PathBuf,
    pub token: Option<String>,
}

/// 启动时扫描 library/ 目录，将已存在的文件/文件夹导入数据库（幂等）。
/// 用于数据库清空后重建元数据，避免重复上传导致文件重命名浪费空间。
async fn scan_library_to_db(state: &AppState) {
    let library_root = state.data_dir.join("library");
    let mut path_to_folder_id: HashMap<PathBuf, Option<i64>> = HashMap::new();
    path_to_folder_id.insert(library_root.clone(), None); // 根目录映射

    // 递归扫描函数
    async fn scan_dir(
        dir: PathBuf,
        parent_folder_id: Option<i64>,
        state: &AppState,
        map: &mut HashMap<PathBuf, Option<i64>>,
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
                        db::create_folder(&state.db, folder_name, parent_folder_id)
                            .await
                            .unwrap_or(0)
                    }
                };
                map.insert(path.clone(), Some(folder_id));
                Box::pin(scan_dir(path, Some(folder_id), state, map)).await;
            } else {
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                let ext = storage::ext_of(file_name);
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
                // 检查是否已存在同名文件（在相同文件夹下）
                let existing =
                    db::find_file_by_name(&state.db, parent_folder_id, file_name).await;
                if let Ok(Some(_)) = existing {
                    continue; // 已存在，跳过
                }
                // 创建文件记录
                if let Ok(file_id) =
                    db::create_file(&state.db, parent_folder_id, file_name, &ext, size as i64).await
                {
                    // 计算相对路径（从 data_dir 开始）
                    let rel = path
                        .strip_prefix(&state.data_dir)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/");
                    let _ = db::insert_version(&state.db, file_id, 1, &rel, size as i64, &sha, "")
                        .await;
                }
            }
        }
    }

    println!("scan: scanning library directory...");
    scan_dir(library_root, None, state, &mut path_to_folder_id).await;
    let folders = path_to_folder_id.len().saturating_sub(1); // 减去根目录映射
    println!("scan: imported {} folders", folders);
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
    for file in &files {
        let Ok(Some(ver)) = db::get_version(&state.db, file.id, file.current_version).await else {
            continue;
        };
        if !ver.blob_path.starts_with("blobs/") {
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
                } else {
                    let _ = std::fs::remove_file(state.data_dir.join(&rel));
                }
            }
            Err(_) => continue,
        }
    }
    println!("migration: migrated {migrated} files to library/, {} skipped", files.len() - migrated);
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
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/plotvault_pdm".into());
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
    };
    scan_library_to_db(&state).await;
    migrate_to_library(&state).await;

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
    axum::serve(listener, app).await?;
    Ok(())
}
