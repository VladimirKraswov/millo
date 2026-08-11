use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = temporary_path(path);
    let backup = backup_path(path);
    remove_if_present(&temporary)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    let had_previous = path.exists();
    if had_previous {
        remove_if_present(&backup)?;
        fs::rename(path, &backup)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        if had_previous {
            let _ = fs::rename(&backup, path);
        }
        return Err(error);
    }

    sync_parent(path)
}

pub fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension("json.tmp")
}

pub fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("json.bak")
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    match path.parent() {
        Some(parent) => File::open(parent)?.sync_all(),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn replaces_the_primary_only_after_a_synced_backup_exists() {
        let path = test_path("replace");
        write_atomically(&path, b"first").unwrap();
        write_atomically(&path, b"second").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read(backup_path(&path)).unwrap(), b"first");
        assert!(!temporary_path(&path).exists());

        cleanup(&path);
    }

    #[test]
    fn removes_a_stale_temporary_file_before_writing() {
        let path = test_path("stale");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(temporary_path(&path), b"partial").unwrap();

        write_atomically(&path, b"valid").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"valid");
        assert!(!temporary_path(&path).exists());

        cleanup(&path);
    }

    fn test_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "millo-storage-{}-{timestamp}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed)
            ))
            .join(format!("{label}.json"))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup_path(path));
        let _ = fs::remove_file(temporary_path(path));
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
