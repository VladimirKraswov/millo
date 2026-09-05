use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static EXPORT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Export user files without sharing the JSON stores' backup names.
/// The destination remains intact until the complete new file has been synced.
pub fn replace_file_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let filename = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "export requires a file name")
    })?;
    let parent = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary_name = filename.to_os_string();
    temporary_name.push(format!(
        ".millo-{}-{}-{}.tmp",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    let temporary = parent.join(temporary_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        super::sync_parent(&parent.join(filename))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_do_not_touch_json_backups_or_other_extensions() {
        let directory = test_directory();
        let nc = directory.join("job.nc");
        let backup = directory.join("job.json.bak");
        fs::write(&backup, "settings backup").unwrap();
        replace_file_atomically(&nc, b"G1 X1").unwrap();
        replace_file_atomically(&directory.join("job.tap"), b"G1 Y1").unwrap();
        replace_file_atomically(&nc, b"G1 X2").unwrap();
        assert_eq!(fs::read_to_string(nc).unwrap(), "G1 X2");
        assert_eq!(fs::read_to_string(backup).unwrap(), "settings backup");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 3);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn failed_replace_preserves_destination_and_cleans_temporary_file() {
        let directory = test_directory();
        let destination = directory.join("job.nc");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), "untouched").unwrap();
        assert!(replace_file_atomically(&destination, b"G1 X1").is_err());
        assert_eq!(
            fs::read_to_string(destination.join("sentinel")).unwrap(),
            "untouched"
        );
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn concurrent_exports_never_share_a_temporary_file() {
        let directory = test_directory();
        let path = directory.join("job.nc");
        std::thread::scope(|scope| {
            for value in 0..8u8 {
                let path = &path;
                scope.spawn(move || replace_file_atomically(path, &vec![value; 8192]).unwrap());
            }
        });
        let result = fs::read(path).unwrap();
        assert_eq!(result.len(), 8192);
        assert!(result.iter().all(|byte| *byte == result[0]));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    fn test_directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "millo-export-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            EXPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
