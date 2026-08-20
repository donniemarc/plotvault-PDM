use sqlx::{FromRow, PgPool, Row};

// 列类型用 TEXT + to_char(now()) 生成 'YYYY-MM-DD HH24:MI:SS' 字符串，
// 与旧 SQLite datetime('now','localtime') 输出格式一致，客户端零改动。
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS folders (
    id BIGSERIAL PRIMARY KEY,
    parent_id BIGINT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT to_char(now(), 'YYYY-MM-DD HH24:MI:SS')
);
CREATE TABLE IF NOT EXISTS files (
    id BIGSERIAL PRIMARY KEY,
    folder_id BIGINT REFERENCES folders(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ext TEXT NOT NULL,
    size BIGINT NOT NULL DEFAULT 0,
    description TEXT NOT NULL DEFAULT '',
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

#[derive(Debug, Clone, serde::Serialize, FromRow)]
pub struct Folder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
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
    Ok(())
}

pub async fn list_folders(pool: &PgPool) -> sqlx::Result<Vec<Folder>> {
    Ok(sqlx::query_as::<_, Folder>(
        "SELECT id, parent_id, name, created_at FROM folders ORDER BY name",
    )
    .fetch_all(pool)
    .await?)
}

pub async fn list_files(pool: &PgPool) -> sqlx::Result<Vec<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, current_version, created_at, updated_at
         FROM files ORDER BY name",
    )
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
        "SELECT id, parent_id, name, created_at FROM folders
         WHERE parent_id IS NOT DISTINCT FROM $1 AND name = $2",
    )
    .bind(parent_id)
    .bind(name)
    .fetch_optional(pool)
    .await?)
}

/// 从根到该文件夹的名称路径（不含根虚拟节点），顺序 根→子
pub async fn folder_path(pool: &PgPool, folder_id: i64) -> sqlx::Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut cur = Some(folder_id);
    while let Some(id) = cur {
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
        "SELECT id, parent_id, name, created_at FROM folders WHERE id = $1",
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
        "SELECT id, folder_id, name, ext, size, description, current_version, created_at, updated_at
         FROM files WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

pub async fn find_file_by_name(
    pool: &PgPool,
    folder_id: Option<i64>,
    name: &str,
) -> sqlx::Result<Option<FileMeta>> {
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, current_version, created_at, updated_at
         FROM files WHERE folder_id IS NOT DISTINCT FROM $1 AND name = $2",
    )
    .bind(folder_id)
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

pub async fn search_files(pool: &PgPool, q: &str) -> sqlx::Result<Vec<FileMeta>> {
    let pattern = format!("%{}%", q);
    Ok(sqlx::query_as::<_, FileMeta>(
        "SELECT id, folder_id, name, ext, size, description, current_version, created_at, updated_at
         FROM files WHERE name ILIKE $1 ORDER BY name",
    )
    .bind(pattern)
    .fetch_all(pool)
    .await?)
}