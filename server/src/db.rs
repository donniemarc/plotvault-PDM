use sqlx::{FromRow, PgPool, Row};
use std::collections::HashSet;

// 列类型用 TEXT + to_char(now()) 生成 'YYYY-MM-DD HH24:MI:SS' 字符串，
// 与旧 SQLite datetime('now','localtime') 输出格式一致，客户端零改动。
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS folders (
    id BIGSERIAL PRIMARY KEY,
    parent_id BIGINT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    code TEXT NOT NULL DEFAULT '',
    stage TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    remarks TEXT NOT NULL DEFAULT '',
    creator TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT to_char(now(), 'YYYY-MM-DD HH24:MI:SS')
);
CREATE TABLE IF NOT EXISTS files (
    id BIGSERIAL PRIMARY KEY,
    folder_id BIGINT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ext TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    description TEXT NOT NULL DEFAULT '',
    code TEXT NOT NULL DEFAULT '',
    stage TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    remarks TEXT NOT NULL DEFAULT '',
    creator TEXT NOT NULL DEFAULT '',
    current_version BIGINT NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT to_char(now(), 'YYYY-MM-DD HH24:MI:SS'),
    updated_at TEXT NOT NULL DEFAULT to_char(now(), 'YYYY-MM-DD HH24:MI:SS')
);
CREATE TABLE IF NOT EXISTS versions (
    id BIGSERIAL PRIMARY KEY,
    file_id BIGINT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    version_no BIGINT NOT NULL,
    blob_path TEXT NOT NULL,
    size BIGINT NOT NULL,
    sha256 TEXT NOT NULL,
    comment TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT to_char(now(), 'YYYY-MM-DD HH24:MI:SS')
);
CREATE INDEX IF NOT EXISTS idx_versions_file ON versions(file_id);
CREATE INDEX IF NOT EXISTS idx_files_folder ON files(folder_id);
"#;

// 迁移语句：为已存在的表添加新列（如果不存在）
const MIGRATIONS: &[&str] = &[
    // folders表新增字段
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS code TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS stage TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS description TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS remarks TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE folders ADD COLUMN IF NOT EXISTS creator TEXT NOT NULL DEFAULT ''",
    // files表新增字段
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS code TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS stage TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS remarks TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS creator TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS drawing_size TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS source_file_type TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS source_file_version TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS other_info TEXT NOT NULL DEFAULT ''",
    "ALTER TABLE files ADD COLUMN IF NOT EXISTS publish_time TEXT NOT NULL DEFAULT ''",
];

#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct Folder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub code: String,
    pub stage: String,
    pub status: String,
    pub description: String,
    pub remarks: String,
    pub creator: String,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct FileMeta {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub ext: String,
    pub size: i64,
    pub description: String,
    pub code: String,
    pub stage: String,
    pub status: String,
    pub remarks: String,
    pub creator: String,
    pub drawing_size: String,
    pub source_file_type: String,
    pub source_file_version: String,
    pub other_info: String,
    pub publish_time: String,
    pub current_version: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct VersionInfo {
    pub id: i64,
    pub file_id: i64,
    pub version_no: i64,
    pub size: i64,
    pub sha256: String,
    pub comment: String,
    pub created_at: String,
    #[serde(skip_serializing)]
    pub blob_path: String,
}

/// 建表（幂等）。应用启动时调用一次。
pub async fn init(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(SCHEMA).execute(pool).await?;
    // 执行迁移语句，为已存在的表添加新列
    for migration in MIGRATIONS {
        if let Err(e) = sqlx::raw_sql(*migration).execute(pool).await {
            eprintln!("warning: migration failed: {} - {}", migration, e);
        }
    }
    Ok(())
}

pub async fn list_folders(pool: &PgPool) -> sqlx::Result<Vec<Folder>> {
    Ok(sqlx::query_as::<_, Folder>(
        "SELECT id, parent_id, name, code, stage, status, description, remarks, creator, created_at FROM folders ORDER BY name",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn list_files(pool: &PgPool) -> sqlx::Result<Vec<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, drawing_size, source_file_type, source_file_version, other_info, publish_time, current_version, created_at, updated_at
         FROM files ORDER BY name",
    )
    .fetch_all(pool)
    .await?)
}

/// 某文件夹（folder_id=None=根目录）下的全部文件
pub async fn list_files_by_folder(pool: &PgPool, folder_id: Option<i64>) -> sqlx::Result<Vec<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, drawing_size, source_file_type, source_file_version, other_info, publish_time, current_version, created_at, updated_at
         FROM files WHERE folder_id IS NOT DISTINCT FROM $1 ORDER BY name",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await?)
}

pub async fn create_folder(pool: &PgPool, name: &str, parent_id: Option<i64>) -> sqlx::Result<i64> {
    let row = sqlx::query("INSERT INTO folders (name, parent_id) VALUES ($1, $2) RETURNING id")
        .bind(name)
        .bind(parent_id)
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>(0))
}

/// 同级下是否存在同名文件夹（用于拒绝重名，避免与真实目录冲突）
pub async fn find_folder_by_name(
    pool: &PgPool,
    parent_id: Option<i64>,
    name: &str,
) -> sqlx::Result<Option<Folder>> {
    Ok(sqlx::query_as::<_, Folder>(
        "SELECT id, parent_id, name, code, stage, status, description, remarks, creator, created_at FROM folders
         WHERE parent_id IS NOT DISTINCT FROM $1 AND name = $2",
    )
    .bind(parent_id)
    .bind(name)
    .fetch_optional(pool)
    .await?)
}

/// 从根到该文件夹的名称路径（不含根虚拟节点），顺序 根→子
/// 包含循环检测：如果发现 parent_id 循环引用，立即终止并返回已收集的路径
pub async fn folder_path(pool: &PgPool, folder_id: i64) -> sqlx::Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut cur = Some(folder_id);
    let mut visited = HashSet::new(); // 循环检测
    const MAX_DEPTH: usize = 50; // 最大深度限制

    while let Some(id) = cur {
        // 循环检测：如果已访问过此 ID，说明存在循环引用
        if !visited.insert(id) {
            eprintln!("warning: folder_path() detected cycle at folder_id={id}, breaking");
            break;
        }
        // 深度限制
        if parts.len() >= MAX_DEPTH {
            eprintln!("warning: folder_path() reached max depth {MAX_DEPTH} at folder_id={id}");
            break;
        }
        match get_folder(pool, id).await? {
            Some(f) => {
                parts.push(f.name);
                cur = f.parent_id;
            }
            None => cur = None,
        }
    }
    parts.reverse();
    Ok(parts)
}

pub async fn get_folder(pool: &PgPool, id: i64) -> sqlx::Result<Option<Folder>> {
    Ok(sqlx::query_as::<_, Folder>(
        "SELECT id, parent_id, name, code, stage, status, description, remarks, creator, created_at FROM folders WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn rename_folder(pool: &PgPool, id: i64, name: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE folders SET name = $1 WHERE id = $2")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_folder_props(
    pool: &PgPool,
    id: i64,
    name: &str,
    code: &str,
    stage: &str,
    status: &str,
    description: &str,
    remarks: &str,
    creator: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE folders SET name = $1, code = $2, stage = $3, status = $4, description = $5, remarks = $6, creator = $7 WHERE id = $8",
    )
    .bind(name)
    .bind(code)
    .bind(stage)
    .bind(status)
    .bind(description)
    .bind(remarks)
    .bind(creator)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn move_folder(pool: &PgPool, id: i64, parent_id: Option<i64>) -> sqlx::Result<()> {
    sqlx::query("UPDATE folders SET parent_id = $1 WHERE id = $2")
        .bind(parent_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// 检查 target_parent_id 是否是 folder_id 的子孙目录（防止循环引用）
pub async fn is_descendant(pool: &PgPool, folder_id: i64, target_parent_id: i64) -> sqlx::Result<bool> {
    let row = sqlx::query(
        "WITH RECURSIVE sub(id) AS (
            SELECT $1::BIGINT
            UNION ALL
            SELECT f.id FROM folders f JOIN sub s ON f.parent_id = s.id
         )
         SELECT EXISTS(SELECT 1 FROM sub WHERE id = $2)",
    )
    .bind(target_parent_id)
    .bind(folder_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<bool, _>(0))
}

pub async fn delete_folder(pool: &PgPool, id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM folders WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Collect all file ids recursively under a folder (including the folder itself is not needed).
pub async fn file_ids_under_folder(pool: &PgPool, folder_id: i64) -> sqlx::Result<Vec<i64>> {
    let rows = sqlx::query(
        "WITH RECURSIVE sub(id) AS (
            SELECT $1::BIGINT
            UNION ALL
            SELECT f.id FROM folders f JOIN sub s ON f.parent_id = s.id
         )
         SELECT f.id FROM files f JOIN sub s ON f.folder_id = s.id",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<i64, _>(0)).collect())
}

pub async fn get_file(pool: &PgPool, id: i64) -> sqlx::Result<Option<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, drawing_size, source_file_type, source_file_version, other_info, publish_time, current_version, created_at, updated_at
         FROM files WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn update_file_props(
    pool: &PgPool,
    id: i64,
    name: &str,
    code: &str,
    stage: &str,
    status: &str,
    description: &str,
    remarks: &str,
    creator: &str,
    drawing_size: &str,
    source_file_type: &str,
    source_file_version: &str,
    other_info: &str,
    publish_time: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE files SET name = $1, code = $2, stage = $3, status = $4, description = $5, remarks = $6, creator = $7, drawing_size = $8, source_file_type = $9, source_file_version = $10, other_info = $11, publish_time = $12, updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $13",
    )
    .bind(name)
    .bind(code)
    .bind(stage)
    .bind(status)
    .bind(description)
    .bind(remarks)
    .bind(creator)
    .bind(drawing_size)
    .bind(source_file_type)
    .bind(source_file_version)
    .bind(other_info)
    .bind(publish_time)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_file_by_name(
    pool: &PgPool,
    folder_id: Option<i64>,
    name: &str,
) -> sqlx::Result<Option<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, current_version, created_at, updated_at
         FROM files WHERE folder_id IS NOT DISTINCT FROM $1 AND name = $2",
    )
    .bind(folder_id)
    .bind(name)
    .fetch_optional(pool)
    .await?)
}

/// 按文件名精确查找文件（不区分文件夹，用于清理UUID重复记录）
pub async fn find_file_by_name_exact(
    pool: &PgPool,
    name: &str,
) -> sqlx::Result<Option<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, current_version, created_at, updated_at
         FROM files WHERE name = $1 LIMIT 1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await?)
}

pub async fn create_file(
    pool: &PgPool,
    folder_id: Option<i64>,
    name: &str,
    ext: &str,
    size: i64,
) -> sqlx::Result<i64> {
    let row = sqlx::query(
        "INSERT INTO files (folder_id, name, ext, size) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(folder_id)
    .bind(name)
    .bind(ext)
    .bind(size)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>(0))
}

pub async fn update_file_size(
    pool: &PgPool,
    file_id: i64,
    size: i64,
    version_no: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE files SET size = $1, current_version = $2,
         updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $3",
    )
    .bind(size)
    .bind(version_no)
    .bind(file_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn next_version_no(pool: &PgPool, file_id: i64) -> sqlx::Result<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(version_no), 0) FROM versions WHERE file_id = $1",
    )
    .bind(file_id)
    .fetch_one(pool)
    .await?;
    Ok(n + 1)
}

pub async fn insert_version(
    pool: &PgPool,
    file_id: i64,
    version_no: i64,
    blob_path: &str,
    size: i64,
    sha256: &str,
    comment: &str,
) -> sqlx::Result<i64> {
    let row = sqlx::query(
        "INSERT INTO versions (file_id, version_no, blob_path, size, sha256, comment)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(file_id)
    .bind(version_no)
    .bind(blob_path)
    .bind(size)
    .bind(sha256)
    .bind(comment)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>(0))
}

/// 更新某版本的存储路径（如版本归档、文件移动后）
pub async fn update_version_blob_path(
    pool: &PgPool,
    file_id: i64,
    version_no: i64,
    blob_path: &str,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE versions SET blob_path = $1 WHERE file_id = $2 AND version_no = $3")
        .bind(blob_path)
        .bind(file_id)
        .bind(version_no)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_versions(pool: &PgPool, file_id: i64) -> sqlx::Result<Vec<VersionInfo>> {
    Ok(sqlx::query_as::<_, VersionInfo>(
        "SELECT id, file_id, version_no, size, sha256, comment, created_at, blob_path
         FROM versions WHERE file_id = $1 ORDER BY version_no ASC",
    )
    .bind(file_id)
    .fetch_all(pool)
    .await?)
}

/// 查找同一文件下是否已有相同 SHA256 的版本（用于上传去重）
pub async fn find_version_by_sha256(
    pool: &PgPool,
    file_id: i64,
    sha256: &str,
) -> sqlx::Result<Option<VersionInfo>> {
    Ok(sqlx::query_as::<_, VersionInfo>(
        "SELECT id, file_id, version_no, size, sha256, comment, created_at, blob_path
         FROM versions WHERE file_id = $1 AND sha256 = $2 LIMIT 1",
    )
    .bind(file_id)
    .bind(sha256)
    .fetch_optional(pool)
    .await?)
}

/// 按 blob_path 查找版本记录（用于扫描去重）
pub async fn find_version_by_blob_path(
    pool: &PgPool,
    blob_path: &str,
) -> sqlx::Result<Option<VersionInfo>> {
    Ok(sqlx::query_as::<_, VersionInfo>(
        "SELECT id, file_id, version_no, size, sha256, comment, created_at, blob_path
         FROM versions WHERE blob_path = $1 LIMIT 1",
    )
    .bind(blob_path)
    .fetch_optional(pool)
    .await?)
}

pub async fn get_version(
    pool: &PgPool,
    file_id: i64,
    version_no: i64,
) -> sqlx::Result<Option<VersionInfo>> {
    Ok(sqlx::query_as::<_, VersionInfo>(
        "SELECT id, file_id, version_no, size, sha256, comment, created_at, blob_path
         FROM versions WHERE file_id = $1 AND version_no = $2",
    )
    .bind(file_id)
    .bind(version_no)
    .fetch_optional(pool)
    .await?)
}

pub async fn delete_file(pool: &PgPool, file_id: i64) -> sqlx::Result<Vec<String>> {
    let rows = sqlx::query("SELECT blob_path FROM versions WHERE file_id = $1")
        .bind(file_id)
        .fetch_all(pool)
        .await?;
    let paths: Vec<String> = rows.iter().map(|r| r.get::<String, _>(0)).collect();
    sqlx::query("DELETE FROM files WHERE id = $1")
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(paths)
}

/// 检查指定 blob_path 是否仍被其它文件的版本引用
pub async fn is_blob_referenced_by_others(
    pool: &PgPool,
    blob_path: &str,
    exclude_file_id: i64,
) -> sqlx::Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM versions WHERE blob_path = $1 AND file_id != $2",
    )
    .bind(blob_path)
    .bind(exclude_file_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn patch_file(
    pool: &PgPool,
    file_id: i64,
    name: Option<&str>,
    folder_id: Option<Option<i64>>,
    description: Option<&str>,
) -> sqlx::Result<()> {
    if let Some(n) = name {
        sqlx::query(
            "UPDATE files SET name = $1, updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $2",
        )
        .bind(n)
        .bind(file_id)
        .execute(pool)
        .await?;
    }
    if let Some(f) = folder_id {
        sqlx::query(
            "UPDATE files SET folder_id = $1, updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $2",
        )
        .bind(f)
        .bind(file_id)
        .execute(pool)
        .await?;
    }
    if let Some(d) = description {
        sqlx::query(
            "UPDATE files SET description = $1, updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $2",
        )
        .bind(d)
        .bind(file_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn patch_file_folder(pool: &PgPool, file_id: i64, folder_id: Option<i64>) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE files SET folder_id = $1, updated_at = to_char(now(), 'YYYY-MM-DD HH24:MI:SS') WHERE id = $2",
    )
    .bind(folder_id)
    .bind(file_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn search_files(pool: &PgPool, q: &str) -> sqlx::Result<Vec<FileMeta>> {
    let pattern = format!("%{}%", q);
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, code, stage, status, remarks, creator, drawing_size, source_file_type, source_file_version, other_info, publish_time, current_version, created_at, updated_at
         FROM files WHERE name ILIKE $1 ORDER BY name",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?)
}