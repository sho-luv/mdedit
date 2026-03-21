use anyhow::Result;
use std::path::Path;

/// Save content to a file atomically using a temp file + rename strategy.
/// This prevents data loss if the process is interrupted during write.
pub fn save_file(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("mdedit-tmp");

    // Write to temp file first
    if let Err(e) = std::fs::write(&tmp, content) {
        // Best-effort cleanup of temp file
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    // Atomically rename temp file to target
    if let Err(e) = std::fs::rename(&tmp, path) {
        // Best-effort cleanup of temp file
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }

    Ok(())
}

/// Load a file's contents. Returns `Some(content)` if the file exists,
/// `None` if the file is not found (it will be created on first save per D-01).
/// Other errors are propagated.
pub fn load_file(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
