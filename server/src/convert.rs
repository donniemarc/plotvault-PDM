use std::path::PathBuf;

use anyhow::Result;

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
