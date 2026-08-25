use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;

use crate::{api::AppError, storage::blob_abs_path, AppState};

/// Convert a DWG blob to DXF using `dwg2dxf` (libredwg). Cached by sha256.
pub async fn dwg_to_dxf(state: &AppState, rel_blob: &str, sha: &str) -> Result<PathBuf, AppError> {
    let cache = state.config_dir.join("dxf_cache").join(format!("{}.dxf", sha));
    if cache.exists() {
        return Ok(cache);
    }

    let src = blob_abs_path(state, rel_blob);
    let out = cache.with_extension("dxf");

    let status = tokio::process::Command::new("dwg2dxf")
        .arg("-o")
        .arg(&out)
        .arg("-y")
        .arg(&src)
        .output()
        .await
        .map_err(|e| AppError::internal(format!("dwg2dxf not available in this environment: {e}")))?;

    if !status.status.success() {
        let msg = String::from_utf8_lossy(&status.stderr).to_string();
        let _ = std::fs::remove_file(&out);
        return Err(AppError::internal(format!(
            "dwg2dxf conversion failed: {}",
            msg.trim()
        )));
    }

    Ok(out)
}

#[derive(Serialize, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// List entries in a ZIP archive (pure Rust).
pub fn list_zip_entries(path: &std::path::Path) -> Result<Vec<ArchiveEntry>, AppError> {
    let file = std::fs::File::open(path)
        .map_err(|e| AppError::internal(format!("cannot open zip: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::internal(format!("invalid zip: {e}")))?;

    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)
            .map_err(|e| AppError::internal(format!("zip entry error: {e}")))?;
        let full_path = entry.name().to_string();
        let is_dir = entry.is_dir();
        let name = full_path
            .trim_end_matches('/')
            .rsplit_once('/')
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| full_path.clone());
        entries.push(ArchiveEntry {
            path: full_path,
            name,
            is_dir,
            size: entry.size(),
        });
    }
    Ok(entries)
}

/// List entries in a RAR archive by shelling out to `unrar l`.
pub async fn list_rar_entries(path: &std::path::Path) -> Result<Vec<ArchiveEntry>, AppError> {
    let output = tokio::process::Command::new("unrar")
        .arg("l")
        .arg("-v")       // verbose: show full paths
        .arg("-p-")      // don't show password prompt
        .arg(path)
        .output()
        .await
        .map_err(|e| AppError::internal(format!("unrar not available: {e}")))?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::internal(format!("unrar list failed: {}", msg.trim())));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    parse_unrar_list(&stdout)
}

/// Parse `unrar l -v` output. Format:
/// ```text
///   Name:              <path>
///   Size:              <size>
///   Packed size:       <packed>
///   ...
/// ```
fn parse_unrar_list(output: &str) -> Result<Vec<ArchiveEntry>, AppError> {
    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_size: u64 = 0;

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Name:") {
            // flush previous entry
            if let Some(path) = current_path.take() {
                let is_dir = path.ends_with('/') || path.ends_with('\\');
                let name = path
                    .trim_end_matches('/')
                    .trim_end_matches('\\')
                    .rsplit_once(['/', '\\'])
                    .map(|(_, n)| n.to_string())
                    .unwrap_or_else(|| path.clone());
                entries.push(ArchiveEntry {
                    path,
                    name,
                    is_dir,
                    size: current_size,
                });
            }
            current_path = Some(rest.trim().to_string());
            current_size = 0;
        } else if let Some(rest) = trimmed.strip_prefix("Size:") {
            current_size = rest.trim().parse().unwrap_or(0);
        }
    }
    // flush last entry
    if let Some(path) = current_path.take() {
        let is_dir = path.ends_with('/') || path.ends_with('\\');
        let name = path
            .trim_end_matches('/')
            .trim_end_matches('\\')
            .rsplit_once(['/', '\\'])
            .map(|(_, n)| n.to_string())
            .unwrap_or_else(|| path.clone());
        entries.push(ArchiveEntry {
            path,
            name,
            is_dir,
            size: current_size,
        });
    }

    Ok(entries)
}

/// Extract a single entry from a ZIP archive to a temp file.
pub fn extract_zip_entry(
    archive_path: &std::path::Path,
    entry_path: &str,
    dest: &std::path::Path,
) -> Result<(), AppError> {
    let file = std::fs::File::open(archive_path)
        .map_err(|e| AppError::internal(format!("cannot open zip: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::internal(format!("invalid zip: {e}")))?;

    let mut entry = archive.by_name(entry_path)
        .map_err(|e| AppError::internal(format!("entry not found: {e}")))?;

    let mut out_file = std::fs::File::create(dest)
        .map_err(|e| AppError::internal(format!("cannot create temp file: {e}")))?;
    std::io::copy(&mut entry, &mut out_file)
        .map_err(|e| AppError::internal(format!("extract error: {e}")))?;

    Ok(())
}

/// Extract a single entry from a RAR archive to a temp file by shelling out to `unrar`.
pub async fn extract_rar_entry(
    archive_path: &std::path::Path,
    entry_path: &str,
    dest: &std::path::Path,
) -> Result<(), AppError> {
    let output = tokio::process::Command::new("unrar")
        .arg("p")
        .arg("-inul")   // no messages, extract to stdout
        .arg(archive_path)
        .arg(entry_path)
        .output()
        .await
        .map_err(|e| AppError::internal(format!("unrar not available: {e}")))?;

    if !output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(AppError::internal(format!("unrar extract failed: {}", msg.trim())));
    }

    std::fs::write(dest, &output.stdout)
        .map_err(|e| AppError::internal(format!("write temp file error: {e}")))?;

    Ok(())
}
