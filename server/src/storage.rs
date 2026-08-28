use std::path::{Path, PathBuf};

use anyhow::Result;
use axum::extract::multipart::Field;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::AppState;

pub fn ext_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, e)) => e.to_lowercase(),
        None => String::new(),
    }
}

pub fn mime_for(ext: &str) -> String {
    let e = ext.to_lowercase();
    let override_mime = match e.as_str() {
        "stl" => Some("model/stl"),
        "3mf" => Some("model/3mf"),
        "dwg" => Some("application/acad"),
        "dxf" => Some("application/dxf"),
        "step" | "stp" => Some("application/step"),
        "iges" | "igs" => Some("model/iges"),
        _ => None,
    };
    override_mime
        .map(|m| m.to_string())
        .unwrap_or_else(|| mime_guess::from_ext(ext).first_or_octet_stream().to_string())
}

pub fn is_dwg(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("dwg")
}

/// 净化目录/文件名：去掉路径分隔符与常见非法字符，避免破坏目录结构
pub fn safe_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let bad = matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || (c as u32) < 32;
        out.push(if bad { '_' } else { c });
    }
    let t = out.trim();
    if t.is_empty() { "_".to_string() } else { t.to_string() }
}

fn rel_of(state: &AppState, abs: &Path) -> Result<String> {
    let rel = abs
        .strip_prefix(&state.data_dir)
        .map_err(|_| anyhow::anyhow!("path outside data dir"))?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// 公共版本的 rel_of，供 main.rs 使用
pub fn rel_of_public(state: &AppState, abs: &Path) -> Result<String> {
    rel_of(state, abs)
}

/// 移动文件：先尝试 rename（同卷快）；失败（如 tmp 在 /config、library 在 /data 跨卷）
/// 则回退为 copy + remove，保证跨挂载卷可用。
pub fn move_file(src: &Path, dest: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(src, dest)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
    }
}

// ---------- 真实目录镜像：library/<文件夹名称路径>/ ----------

pub fn library_root(state: &AppState) -> PathBuf {
    state.data_dir.join("library")
}

/// 某个文件夹（名称路径 parts，空=根目录）在 library 下的目录
pub fn folder_dir(state: &AppState, parts: &[String]) -> PathBuf {
    let mut p = library_root(state);
    for part in parts {
        p = p.join(safe_name(part));
    }
    p
}

pub fn ensure_folder_dir(state: &AppState, parts: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(folder_dir(state, parts))
}

/// 删除整个文件夹目录树（递归）
pub fn remove_folder_dir(state: &AppState, parts: &[String]) {
    let _ = std::fs::remove_dir_all(folder_dir(state, parts));
}

/// 重命名/移动文件夹目录（先确保父目录存在）
pub fn rename_folder_dir(state: &AppState, old_parts: &[String], new_parts: &[String]) -> std::io::Result<()> {
    let old = folder_dir(state, old_parts);
    let new = folder_dir(state, new_parts);
    if old == new {
        return Ok(());
    }
    if let Some(parent) = new.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if old.exists() {
        std::fs::rename(&old, &new)
    } else {
        std::fs::create_dir_all(&new)
    }
}

/// 新文件落到 library 对应目录（真实文件），返回相对路径。
/// 如果目标已存在则直接覆盖（旧版本应在调用前已归档到 blobs/）。
pub fn finalize_library_file(
    state: &AppState,
    folder_parts: &[String],
    file_name: &str,
    tmp: &Path,
) -> Result<String> {
    let dir = folder_dir(state, folder_parts);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(safe_name(file_name));
    move_file(tmp, &dest)?;
    rel_of(state, &dest)
}

/// 把当前版本（library 下）归档到 blobs/<file_id>/，保留版本历史
pub fn archive_blob(state: &AppState, file_id: i64, version_no: i64, ext: &str, current_rel: &str) -> Result<String> {
    let src = state.data_dir.join(current_rel);
    if !src.exists() {
        return Ok(current_rel.to_string());
    }
    let dir = state.data_dir.join("blobs").join(file_id.to_string());
    std::fs::create_dir_all(&dir)?;
    let name = format!("{}_{}.{}", version_no, Uuid::new_v4().simple(), ext);
    let dst = dir.join(&name);
    move_file(&src, &dst)?;
    Ok(format!("blobs/{}/{}", file_id, name))
}

/// 移动/重命名 library 中的文件到新目录（文件重命名、移动文件夹）。
/// 如果目标已存在则直接覆盖（旧版本应在调用前已归档到 blobs/）。
pub fn move_library_file(
    state: &AppState,
    old_rel: &str,
    folder_parts: &[String],
    new_name: &str,
) -> Result<String> {
    let src = state.data_dir.join(old_rel);
    if !src.exists() {
        return Ok(old_rel.to_string());
    }
    let dir = folder_dir(state, folder_parts);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(safe_name(new_name));
    if dest != src {
        move_file(&src, &dest)?;
    }
    rel_of(state, &dest)
}

/// 迁移用：把已有 blob 复制一份到 library 目录（如果目标已存在则跳过）
pub fn copy_to_library(
    state: &AppState,
    src_rel: &str,
    folder_parts: &[String],
    file_name: &str,
) -> Result<String> {
    let src = state.data_dir.join(src_rel);
    if !src.exists() {
        return Err(anyhow::anyhow!("source blob missing"));
    }
    let dir = folder_dir(state, folder_parts);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(safe_name(file_name));
    if dest.exists() {
        // 目标文件已存在，跳过复制，直接返回现有文件的相对路径
        return rel_of(state, &dest);
    }
    std::fs::copy(&src, &dest)?;
    rel_of(state, &dest)
}

/// Stream a multipart file field to a temp file. Returns (tmp_path, sha256, size).
pub async fn stream_field_to_temp(state: &AppState, field: Field<'_>) -> Result<(PathBuf, String, u64)> {
    let tmp_dir = state.config_dir.join("tmp");
    let tmp_path = tmp_dir.join(format!("up_{}.part", Uuid::new_v4().simple()));
    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut hasher = Sha256::new();
    let mut size: u64 = 0;

    let mut field = field;
    while let Some(chunk) = field.chunk().await? {
        hasher.update(&chunk);
        size += chunk.len() as u64;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;

    let sha = hex::encode(hasher.finalize());
    Ok((tmp_path, sha, size))
}

pub fn blob_abs_path(state: &AppState, rel: &str) -> PathBuf {
    state.data_dir.join(rel)
}

/// 删除版本文件；仅尝试删除其所在空目录（非递归，避免误删 library 中的其它文件）
pub fn remove_blobs(state: &AppState, rel_paths: &[String]) {
    for rel in rel_paths {
        let path = state.data_dir.join(rel);
        let _ = std::fs::remove_file(path);
        if let Some(dir) = rel.rsplit_once('/').map(|(d, _)| d.to_string()) {
            let dir = state.data_dir.join(dir);
            let _ = std::fs::remove_dir(dir);
        }
    }
}

pub fn remove_file_blobs(state: &AppState, file_id: i64) {
    let dir = state.data_dir.join("blobs").join(file_id.to_string());
    let _ = std::fs::remove_dir_all(dir);
}
