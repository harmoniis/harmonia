//! Patch writing — writes diff files for evolution application.
//! Pure functional: state passed in, no globals.

use crate::OuroborosState;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs()
}

/// Write a patch file and record to chronicle. Returns the file path.
pub fn write_patch(state: &OuroborosState, component: &str, body: &str) -> Result<String, String> {
    let dir = std::path::Path::new(&state.patch_dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("create patch dir: {e}"))?;
    let filename = format!("{}-{}.patch", component.replace('/', "_"), now_secs());
    let path = dir.join(&filename);
    std::fs::write(&path, body).map_err(|e| format!("write patch: {e}"))?;
    let _ = harmonia_chronicle::ouroboros::record(
        "patch_write", Some(component), None, Some(body.len() as i64), true,
    );
    Ok(path.to_string_lossy().into_owned())
}
