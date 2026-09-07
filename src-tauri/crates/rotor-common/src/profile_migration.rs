//! Offline, additive profile migration. Existing profiles and backups are never replaced.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileDigest {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub source_version: String,
    pub importer_version: String,
    pub created_unix_seconds: u64,
    pub files: BTreeMap<String, FileDigest>,
}
type Result<T> = std::result::Result<T, String>;
const RECEIPT: &str = "migration-receipt.json";

fn digest(path: &Path) -> Result<FileDigest> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(FileDigest {
        bytes,
        sha256: format!("{:x}", hash.finalize()),
    })
}
fn collect(root: &Path, path: &Path, result: &mut BTreeMap<String, FileDigest>) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Profile symlinks are not imported".into());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            collect(root, &entry.map_err(|e| e.to_string())?.path(), result)?;
        }
    } else if metadata.is_file() {
        let name = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Profile path is not Unicode")?
            .replace('\\', "/");
        result.insert(name, digest(path)?);
    } else {
        return Err("Profile contains a non-regular file".into());
    }
    Ok(())
}
fn inventory(root: &Path) -> Result<BTreeMap<String, FileDigest>> {
    let mut files = BTreeMap::new();
    for name in ["config.toml", "shotter"] {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => collect(root, &path, &mut files)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(files)
}
fn new_path(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .ok_or("Destination must name a new directory")?;
    let parent = path
        .parent()
        .ok_or("Destination must have a parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let result = parent.join(name);
    if fs::symlink_metadata(&result).is_ok() {
        return Err("Destination already exists; select a new directory".into());
    }
    Ok(result)
}
fn copy_files(
    source: &Path,
    destination: &Path,
    files: &BTreeMap<String, FileDigest>,
) -> Result<()> {
    for (name, expected) in files {
        let relative = Path::new(name);
        if relative
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Invalid profile inventory path".into());
        }
        let target = destination.join(relative);
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let original = source.join(relative);
        if fs::symlink_metadata(&original)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Profile changed while copying".into());
        }
        if !original
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(source)
        {
            return Err("Profile path escaped its source directory".into());
        }
        let mut input = fs::File::open(original).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        if &digest(&target)? != expected {
            return Err("Profile changed while copying; backup retained".into());
        }
    }
    Ok(())
}
fn write_receipt(root: &Path, receipt: &Receipt) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(receipt).map_err(|e| e.to_string())?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(RECEIPT))
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
pub fn verify(root: &Path) -> Result<Receipt> {
    let receipt: Receipt =
        serde_json::from_slice(&fs::read(root.join(RECEIPT)).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if inventory(root)? != receipt.files {
        return Err("Profile snapshot checksum mismatch".into());
    }
    Ok(receipt)
}
fn validate(root: &Path) -> Result<()> {
    let config = root.join("config.toml");
    if config.is_file() {
        let text = fs::read_to_string(config).map_err(|e| e.to_string())?;
        // Do not include parser excerpts: config may contain API credentials.
        toml::from_str::<crate::Config>(&text)
            .map_err(|_| "Invalid config.toml; original and backup are preserved")?;
    }
    let record = root.join("shotter/record.toml");
    if record.is_file() {
        let text = fs::read_to_string(record).map_err(|e| e.to_string())?;
        let value: toml::Value = toml::from_str(&text)
            .map_err(|_| "Invalid record.toml; original and backup are preserved")?;
        if value
            .get("workspaces")
            .is_some_and(|value| !value.is_table())
        {
            return Err("Invalid record workspaces; backup preserved".into());
        }
    }
    Ok(())
}
pub fn backup_profile(source: &Path, backup: &Path, source_version: &str) -> Result<Receipt> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let backup = new_path(backup)?;
    if !source.is_dir() || backup.starts_with(&source) {
        return Err("Backup must be separate from its source profile".into());
    }
    if source_version.trim().is_empty() {
        return Err("Record the source version (or unknown)".into());
    }
    let files = inventory(&source)?;
    if files.is_empty() {
        return Err("Source has no Rotor configuration or screenshot records".into());
    }
    let builder = fs::DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder.create(&backup).map_err(|e| e.to_string())?;
    copy_files(&source, &backup, &files)?;
    if inventory(&source)? != files {
        return Err("Source changed during backup; retry with the app closed".into());
    }
    let receipt = Receipt {
        files,
        source_version: source_version.into(),
        importer_version: env!("CARGO_PKG_VERSION").into(),
        created_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs(),
    };
    write_receipt(&backup, &receipt)?;
    Ok(receipt)
}

/// Called with both native and legacy instance leases held, before first native writes.
pub fn prepare_native_profile(directory: &Path) -> Result<Option<PathBuf>> {
    #[derive(Serialize, Deserialize)]
    struct Marker {
        schema: u32,
        initial_native_version: String,
        backup: Option<PathBuf>,
    }
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    let marker = directory.join(".native-migration.json");
    if marker.exists() {
        if fs::metadata(&marker).map_err(|e| e.to_string())?.len() > 65536 {
            return Err("Native migration marker is too large".into());
        }
        let value: Marker = serde_json::from_slice(&fs::read(&marker).map_err(|e| e.to_string())?)
            .map_err(|_| "Invalid native migration marker; profile preserved")?;
        if value.schema != 1 {
            return Err("Unsupported native migration marker".into());
        }
        return Ok(value.backup);
    }
    let backup = if inventory(&directory)?.is_empty() {
        None
    } else {
        let parent = directory
            .parent()
            .ok_or("Profile has no backup parent directory")?;
        let root = tempfile::Builder::new()
            .prefix(".rotor-backup-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?
            .keep();
        let backup = root.join("data");
        backup_profile(
            &directory,
            &backup,
            "legacy profile; source version unknown",
        )?;
        validate(&backup).map_err(|error| format!("{error}. Backup: {}", backup.display()))?;
        Some(backup)
    };
    let value = Marker {
        schema: 1,
        initial_native_version: env!("CARGO_PKG_VERSION").into(),
        backup: backup.clone(),
    };
    crate::persistence::atomic_write_private(
        &marker,
        &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(backup)
}

/// Import only configuration and shotter data; indexes are rebuilt by the app.
/// The source app must be closed. Hash checks also detect changes during copying.
pub fn import(
    source: &Path,
    destination: &Path,
    backup: &Path,
    source_version: &str,
) -> Result<Receipt> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let destination = new_path(destination)?;
    let backup = new_path(backup)?;
    if !source.is_dir()
        || destination.starts_with(&source)
        || backup.starts_with(&source)
        || destination.starts_with(&backup)
        || backup.starts_with(&destination)
    {
        return Err("Source, backup and destination must be separate directories".into());
    }
    let receipt = backup_profile(&source, &backup, source_version)?;
    validate(&backup)?;
    let staging = tempfile::Builder::new()
        .prefix(".rotor-import-")
        .tempdir_in(destination.parent().unwrap())
        .map_err(|e| e.to_string())?;
    copy_files(&backup, staging.path(), &receipt.files)?;
    write_receipt(staging.path(), &receipt)?;
    verify(staging.path())?;
    if fs::symlink_metadata(&destination).is_ok() {
        return Err("Destination appeared during import; backup retained".into());
    }
    fs::rename(staging.path(), &destination)
        .map_err(|e| format!("Cannot publish imported profile; backup retained: {e}"))?;
    let _ = staging.keep();
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_native_use_keeps_one_backup_and_does_not_modify_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("profile");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("config.toml"), "theme = '1'\nfuture = 'keep'").unwrap();
        let original = fs::read(source.join("config.toml")).unwrap();
        let backup = prepare_native_profile(&source).unwrap().unwrap();
        assert_eq!(
            prepare_native_profile(&source).unwrap(),
            Some(backup.clone())
        );
        verify(&backup).unwrap();
        assert_eq!(fs::read(source.join("config.toml")).unwrap(), original);
        assert_eq!(fs::read(backup.join("config.toml")).unwrap(), original);
    }
    #[test]
    fn import_keeps_unknown_keys_all_workspaces_and_original_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("shotter/other")).unwrap();
        fs::write(
            source.join("config.toml"),
            "theme = '1'\nfuture_key = 'retained'\nquick_actions = '[]'\n",
        )
        .unwrap();
        fs::write(
            source.join("shotter/record.toml"),
            "[workspaces.other.shotters]\n",
        )
        .unwrap();
        fs::write(source.join("shotter/other/1.png"), b"preserved bytes").unwrap();
        fs::write(source.join("index-cache"), b"obsolete cache").unwrap();
        let destination = temp.path().join("native");
        let backup = temp.path().join("backup");
        let original = inventory(&source).unwrap();
        import(&source, &destination, &backup, "2.6.0").unwrap();
        assert_eq!(inventory(&source).unwrap(), original);
        assert_eq!(verify(&backup).unwrap().files, original);
        assert_eq!(verify(&destination).unwrap().files, original);
        assert!(!destination.join("index-cache").exists());
        assert!(import(
            &source,
            &destination,
            &temp.path().join("another-backup"),
            "2.6.0"
        )
        .is_err());
        assert!(!temp.path().join("another-backup").exists());
        fs::write(destination.join("config.toml"), "theme = '2'").unwrap();
        assert!(verify(&destination).is_err());
        assert_eq!(verify(&backup).unwrap().files, original);
    }
    #[test]
    fn corrupt_config_retains_backup_without_publishing_or_leaking_values() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("config.toml"),
            "api_key = 'private unclosed secret",
        )
        .unwrap();
        let destination = temp.path().join("native");
        let backup = temp.path().join("backup");
        let error = import(&source, &destination, &backup, "unknown").unwrap_err();
        assert!(!error.contains("secret"));
        assert!(!destination.exists());
        verify(&backup).unwrap();
        assert_eq!(
            fs::read(source.join("config.toml")).unwrap(),
            fs::read(backup.join("config.toml")).unwrap()
        );
    }
    #[test]
    fn nested_destinations_are_rejected_before_copying() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("config.toml"), "theme = '1'").unwrap();
        assert!(import(
            temp.path(),
            &temp.path().join("nested"),
            &temp.path().join("backup"),
            "2.6.0"
        )
        .is_err());
        assert!(!temp.path().join("backup").exists());
    }
}
