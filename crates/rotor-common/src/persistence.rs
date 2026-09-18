use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// Replace one file only after its complete contents have been written and
/// flushed. A failed write/rename leaves the original file untouched.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_impl(path, false, |file| file.write_all(bytes))
}

pub fn atomic_write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    atomic_write_private_with(path, |file| file.write_all(bytes))
}

/// Stream large private artifacts without allocating a second in-memory copy.
pub fn atomic_write_private_with(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    atomic_write_impl(path, true, write)
}

fn atomic_write_impl(
    path: &Path,
    private: bool,
    write: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory")
    })?;
    fs::create_dir_all(parent)?;
    for _ in 0..16 {
        let mut name = OsString::from(".");
        name.push(
            path.file_name()
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file has no name"))?,
        );
        name.push(format!(
            ".{}.{}.tmp",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let result = (|| {
            write(&mut file)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        } else {
            sweep_stale_temporaries(parent, path);
        }
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "temporary file collision limit reached",
    ))
}

/// Remove `.{name}.{pid}.{seq}.tmp` files that earlier, crashed processes left
/// beside a file this process has just replaced. Files of the current process
/// may belong to a concurrent write and are kept; removal failures only warn.
fn sweep_stale_temporaries(parent: &Path, path: &Path) {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let prefix = format!(".{name}.");
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        let Some(middle) = file_name
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix(".tmp"))
        else {
            continue;
        };
        let Some((pid, sequence)) = middle.split_once('.') else {
            continue;
        };
        let Ok(pid) = pid.parse::<u32>() else {
            continue;
        };
        if pid == std::process::id()
            || sequence.is_empty()
            || !sequence.bytes().all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        if let Err(error) = fs::remove_file(entry.path()) {
            log::warn!(
                "Cannot remove stale temporary file {}: {error}",
                entry.path().display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_replacement_sweeps_only_other_processes_temporaries() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        let other_pid = std::process::id().wrapping_add(1);
        let stale = directory
            .path()
            .join(format!(".config.toml.{other_pid}.7.tmp"));
        let own = directory
            .path()
            .join(format!(".config.toml.{}.999.tmp", std::process::id()));
        let unrelated = directory
            .path()
            .join(format!(".other.toml.{other_pid}.0.tmp"));
        let longer_name = directory
            .path()
            .join(format!(".config.toml.bak.{other_pid}.0.tmp"));
        for file in [&stale, &own, &unrelated, &longer_name] {
            fs::write(file, b"leftover").unwrap();
        }
        atomic_write(&path, b"new").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert!(!stale.exists());
        for kept in [&own, &unrelated, &longer_name] {
            assert_eq!(fs::read(kept).unwrap(), b"leftover");
        }
        // A failed replacement leaves stale files for the next success.
        let blocked = directory.path().join("blocked.toml");
        let blocked_stale = directory
            .path()
            .join(format!(".blocked.toml.{other_pid}.7.tmp"));
        fs::write(&blocked_stale, b"leftover").unwrap();
        fs::create_dir(&blocked).unwrap();
        assert!(atomic_write(&blocked, b"new").is_err());
        assert_eq!(fs::read(&blocked_stale).unwrap(), b"leftover");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 6);
    }

    #[cfg(unix)]
    #[test]
    fn private_replacements_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private.json");
        fs::write(&path, b"old").unwrap();
        atomic_write_private(&path, b"new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(path).unwrap(), b"new");
    }
}
