use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Take, Write},
    path::{Path, PathBuf},
};

use super::super::excluded_dirs::ExcludedDirs;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn check_cancel(cancel: Option<&AtomicBool>) -> io::Result<()> {
    if cancel.is_some_and(|cancel| cancel.load(Ordering::Acquire)) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Index operation cancelled",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn index_path(drive: &str, excluded: &ExcludedDirs) -> PathBuf {
    #[cfg(not(test))]
    let root = rotor_common::file_path::get_userdata_path().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("rotor-index-{}", std::process::id()))
    });
    #[cfg(test)]
    let root = std::env::temp_dir().join(format!("rotor-search-tests-{}", std::process::id()));
    path_in(&root, drive, excluded)
}

fn path_in(root: &Path, drive: &str, excluded: &ExcludedDirs) -> PathBuf {
    let mut hash = Sha256::new();
    hash.update(drive.as_bytes());
    hash.update([0]);
    hash.update(excluded.cache_identity().as_bytes());
    root.join(".search-index")
        .join(format!("{:x}.idx", hash.finalize()))
}

struct HashWriter<W> {
    inner: W,
    hash: Sha256,
}

impl<W: Write> Write for HashWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(bytes)?;
        self.hash.update(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(super) fn write(
    path: &Path,
    body: impl FnOnce(&mut dyn Write) -> io::Result<()>,
) -> io::Result<()> {
    rotor_common::persistence::atomic_write_private_with(path, |file| {
        let mut writer = HashWriter {
            inner: BufWriter::new(file),
            hash: Sha256::new(),
        };
        body(&mut writer)?;
        writer.inner.write_all(&writer.hash.finalize())?;
        writer.inner.flush()
    })
}

/// Validate the complete snapshot before constructing the index. Verification
/// uses a fixed buffer rather than retaining serialized and decoded copies.
#[cfg(test)]
pub(super) fn read(path: &Path) -> io::Result<BufReader<Take<File>>> {
    read_with_cancel(path, None)
}

pub(super) fn read_with_cancel(
    path: &Path,
    cancel: Option<&AtomicBool>,
) -> io::Result<BufReader<Take<File>>> {
    check_cancel(cancel)?;
    let mut file = File::open(path)?;
    let length = file
        .metadata()?
        .len()
        .checked_sub(32)
        .ok_or_else(|| invalid("Truncated index checksum"))?;
    let mut hash = Sha256::new();
    let mut remaining = length;
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        check_cancel(cancel)?;
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        hash.update(&buffer[..count]);
        remaining -= count as u64;
    }
    let mut checksum = [0u8; 32];
    file.read_exact(&mut checksum)?;
    if hash.finalize().as_slice() != checksum {
        return Err(invalid("Index checksum mismatch"));
    }
    file.seek(SeekFrom::Start(0))?;
    Ok(BufReader::new(file.take(length)))
}

pub(super) fn end(reader: &mut impl Read) -> io::Result<()> {
    let mut byte = [0];
    if reader.read(&mut byte)? != 0 {
        return Err(invalid("Unexpected trailing index data"));
    }
    Ok(())
}

pub(super) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_roots_and_exclusions_have_distinct_cache_paths() {
        let a = ExcludedDirs::default();
        let b = super::super::super::excluded_dirs::parse_excluded_dirs("target", None);
        let root = Path::new("synthetic");
        assert_ne!(path_in(root, "C", &a), path_in(root, "C", &b));
        assert_ne!(path_in(root, "/a/b", &a), path_in(root, "/a_b", &a));
        assert_ne!(path_in(root, "C", &a), path_in(&root.join("dev"), "C", &a));
    }

    #[test]
    fn interrupted_write_preserves_snapshot_and_removes_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index");
        write(&path, |writer| writer.write_all(b"original")).unwrap();
        assert!(write(&path, |writer| {
            writer.write_all(b"partial")?;
            Err(io::Error::other("synthetic failure"))
        })
        .is_err());
        let mut bytes = Vec::new();
        read(&path).unwrap().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn detects_corruption_and_every_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index");
        write(&path, |writer| writer.write_all(b"snapshot")).unwrap();
        let original = std::fs::read(&path).unwrap();
        for length in 0..original.len() {
            std::fs::write(&path, &original[..length]).unwrap();
            assert!(read(&path).is_err());
        }
        for index in 0..original.len() {
            let mut corrupt = original.clone();
            corrupt[index] ^= 1;
            std::fs::write(&path, corrupt).unwrap();
            assert!(read(&path).is_err());
        }
    }
}
