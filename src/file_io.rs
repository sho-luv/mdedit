use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Save content to a file atomically (temp file + rename), so an interrupted
/// write can never leave a half-written document.
///
/// Symlinks are followed (the link stays a link), and an existing file's
/// permissions are kept. The temp file never exposes the content more widely
/// than the original: it is created private and given the original's
/// permissions before anything is written to it.
pub fn save_file(path: &Path, content: &str) -> Result<()> {
    // Write through symlinks instead of replacing the link with a file.
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let tmp = temp_path(&target);
    let existing = std::fs::metadata(&target).ok().map(|m| m.permissions());

    let write = || -> std::io::Result<()> {
        let mut file = create_exclusive(&tmp, existing.is_some())?;
        if let Some(permissions) = existing.clone() {
            file.set_permissions(permissions)?;
        }
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, &target)
    };
    if let Err(e) = write() {
        let _ = std::fs::remove_file(&tmp); // best-effort cleanup
        return Err(e.into());
    }
    Ok(())
}

/// Create `path` for writing, failing rather than opening anything that is
/// already there — so a symlink planted at the temp path can't redirect the
/// write. A leftover temp file from a crashed save is removed (the entry
/// itself, never what it points to) and creation is retried once.
///
/// `private` creates the file readable only by the owner (the caller then
/// applies the original file's permissions); otherwise the usual
/// umask-controlled default applies, as for any new file.
fn create_exclusive(path: &Path, private: bool) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if private { 0o600 } else { 0o666 });
    }
    #[cfg(not(unix))]
    let _ = private;
    match options.open(path) {
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(path)?;
            options.open(path)
        }
        result => result,
    }
}

/// Hidden sibling temp file: `dir/.name.mdedit-tmp`. Being in the same
/// directory keeps the rename atomic; keeping the full name avoids clashes
/// between `a.md` and `a.txt`.
fn temp_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled".to_string());
    target.with_file_name(format!(".{name}.mdedit-tmp"))
}

/// Load a file's contents. Returns `Some(content)` if the file exists,
/// `None` if the file is not found (it will be created on first save per D-01).
/// Other problems are reported with the path and a plain-language reason.
pub fn load_file(path: &Path) -> Result<Option<String>> {
    if path.is_dir() {
        bail!("{} is a directory, not a file", path.display());
    }
    match std::fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(content) => Ok(Some(content)),
            Err(_) => bail!(
                "{} is not UTF-8 text (is it a binary file?)",
                path.display()
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("cannot open {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mdedit-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_reports_directories_and_binary_files_plainly() {
        let dir = scratch("load");
        assert!(load_file(&dir.join("new.md")).unwrap().is_none());

        let err = load_file(&dir).unwrap_err().to_string();
        assert!(err.contains("is a directory"), "{err}");

        let bin = dir.join("bin.md");
        std::fs::write(&bin, [0xff, 0xfe, 0x00, 0x80]).unwrap();
        let err = load_file(&bin).unwrap_err().to_string();
        assert!(err.contains("not UTF-8 text"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn save_keeps_permissions_and_follows_symlinks() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("save");
        let real = dir.join("real.md");
        std::fs::write(&real, "old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = dir.join("link.md");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        save_file(&link, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        let mode = std::fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // No temp files left behind.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A symlink planted where the temp file goes must not redirect the
    /// write to another file.
    #[cfg(unix)]
    #[test]
    fn save_does_not_write_through_a_planted_temp_symlink() {
        let dir = scratch("planted");
        let doc = dir.join("doc.md");
        std::fs::write(&doc, "old").unwrap();
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, "untouched").unwrap();
        std::os::unix::fs::symlink(&victim, temp_path(&std::fs::canonicalize(&doc).unwrap()))
            .unwrap();

        save_file(&doc, "new").unwrap();

        assert_eq!(std::fs::read_to_string(&doc).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "untouched");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// New files get the normal default permissions; the temp file for an
    /// existing private file is never created more open than the original.
    #[cfg(unix)]
    #[test]
    fn temp_file_for_an_existing_file_starts_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("private");
        let tmp = dir.join(".x.mdedit-tmp");
        let file = create_exclusive(&tmp, true).unwrap();
        let mode = file.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(dir);
    }
}
