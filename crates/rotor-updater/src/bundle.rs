//! Portable validation/extraction and rollback mechanics for macOS app updates.
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
const MAX_EXPANDED: u64 = 3 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 20000;

pub struct PreparedBundle {
    _temporary: tempfile::TempDir,
    pub app: PathBuf,
}
#[derive(Debug)]
pub struct BundleInfo {
    pub identifier: String,
    pub version: semver::Version,
    pub executable: PathBuf,
}

pub fn inspect(app: &Path) -> Result<BundleInfo> {
    if app.extension().and_then(|s| s.to_str()) != Some("app") {
        return Err("Expected an app bundle".into());
    }
    if fs::symlink_metadata(app)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("App bundle may not be a symlink".into());
    }
    let canonical = app.canonicalize().map_err(|e| e.to_string())?;
    if !app
        .join("Contents/Info.plist")
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&canonical)
    {
        return Err("Bundle metadata escapes the app".into());
    }
    for relative in [
        "Contents/Resources/assets/model/pp-ocrv6_tiny_det.onnx",
        "Contents/Resources/assets/model/pp-ocrv6_tiny_rec.onnx",
        "Contents/Resources/assets/model/ppocrv6_tiny_dict.txt",
    ] {
        let resource = app
            .join(relative)
            .canonicalize()
            .map_err(|_| format!("Missing bundled resource {relative}"))?;
        if !resource.starts_with(&canonical)
            || !resource.is_file()
            || fs::metadata(resource).map_err(|e| e.to_string())?.len() == 0
        {
            return Err(format!("Invalid bundled resource {relative}"));
        }
    }
    let metadata =
        plist::Value::from_file(app.join("Contents/Info.plist")).map_err(|e| e.to_string())?;
    let values = metadata.as_dictionary().ok_or("Invalid bundle metadata")?;
    let string = |key: &str| {
        values
            .get(key)
            .and_then(plist::Value::as_string)
            .ok_or_else(|| format!("Missing bundle field {key}"))
    };
    let identifier = string("CFBundleIdentifier")?.to_string();
    if !matches!(
        identifier.as_str(),
        "cc.fluctus.rotor3" | "cc.fluctus.rotor3.dev"
    ) {
        return Err("Bundle is not Rotor".into());
    }
    let executable_name = if identifier == "cc.fluctus.rotor3" {
        "rotor"
    } else {
        "rotor-desktop"
    };
    if string("CFBundleExecutable")? != executable_name {
        return Err("Bundle has an unexpected executable".into());
    }
    let full_version = values
        .get("RotorVersion")
        .and_then(plist::Value::as_string)
        .unwrap_or(string("CFBundleShortVersionString")?);
    let version = semver::Version::parse(full_version).map_err(|e| e.to_string())?;
    let executable = app.join("Contents/MacOS").join(executable_name);
    if !executable
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(&canonical)
    {
        return Err("Bundle executable escapes the app".into());
    }
    let mut header = [0u8; 8];
    fs::File::open(&executable)
        .map_err(|e| e.to_string())?
        .read_exact(&mut header)
        .map_err(|e| e.to_string())?;
    if header[..4] != [0xcf, 0xfa, 0xed, 0xfe]
        || u32::from_le_bytes(header[4..8].try_into().unwrap()) != 0x0100000c
    {
        return Err("Expected an arm64 Mach-O executable".into());
    }
    Ok(BundleInfo {
        identifier,
        version,
        executable,
    })
}

fn extract(reader: impl Read, destination: &Path) -> Result<PathBuf> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(reader));
    let mut root = None;
    let mut expanded = 0u64;
    for (index, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        if index >= MAX_ENTRIES {
            return Err("Update archive contains too many entries".into());
        }
        let mut entry = entry.map_err(|e| e.to_string())?;
        let relative = entry.path().map_err(|e| e.to_string())?.into_owned();
        if relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("Unsafe archive path".into());
        }
        let first = relative
            .components()
            .next()
            .ok_or("Empty archive path")?
            .as_os_str()
            .to_os_string();
        if Path::new(&first).extension().and_then(|s| s.to_str()) != Some("app") {
            return Err("Archive must contain one app bundle".into());
        }
        if root.as_ref().is_some_and(|root| root != &first) {
            return Err("Archive contains multiple roots".into());
        }
        root = Some(first);
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err("Archive links and special files are not accepted".into());
        }
        let size = entry.size();
        expanded = expanded.checked_add(size).ok_or("Archive size overflow")?;
        if expanded > MAX_EXPANDED {
            return Err("Expanded update is too large".into());
        }
        let output = destination.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&output).map_err(|e| e.to_string())?;
            continue;
        }
        fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .map_err(|e| e.to_string())?;
        let copied = std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
        if copied != size {
            return Err("Truncated archive entry".into());
        }
        file.flush().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let executable = entry.header().mode().map_err(|e| e.to_string())? & 0o111 != 0;
            fs::set_permissions(
                &output,
                fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let app = destination.join(root.ok_or("Empty update archive")?);
    #[cfg(unix)]
    fn directory_permissions(path: &Path) -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                directory_permissions(&entry.path())?;
            }
        }
        Ok(())
    }
    #[cfg(unix)]
    directory_permissions(&app)?;
    Ok(app)
}

pub fn prepare(
    archive: &Path,
    signature: &str,
    version: &str,
    identifier: &str,
    parent: &Path,
) -> Result<PreparedBundle> {
    let temporary = tempfile::Builder::new()
        .prefix(".rotor-update-")
        .tempdir_in(parent)
        .map_err(|e| e.to_string())?;
    let copy = temporary.path().join("verified.app.tar.gz");
    let mut input = fs::File::open(archive)
        .map_err(|e| e.to_string())?
        .take(crate::MAX_DOWNLOAD + 1);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&copy)
        .map_err(|e| e.to_string())?;
    if std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())? > crate::MAX_DOWNLOAD {
        return Err("Update archive is too large".into());
    }
    output.sync_all().map_err(|e| e.to_string())?;
    drop(output);
    crate::verify_file(&copy, signature, crate::PUBLIC_KEY)?;
    let content = temporary.path().join("content");
    fs::create_dir(&content).map_err(|e| e.to_string())?;
    let app = extract(fs::File::open(copy).map_err(|e| e.to_string())?, &content)?;
    let info = inspect(&app)?;
    if info.identifier != identifier || info.version.to_string() != version {
        return Err("Bundle identity/version differs from the signed update selection".into());
    }
    Ok(PreparedBundle {
        _temporary: temporary,
        app,
    })
}

/// Call only after the old process exits. Backups and failed new bundles are retained.
pub fn replace_and_launch(
    new: &Path,
    destination: &Path,
    backup: &Path,
    failed: &Path,
    launch: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    let current = inspect(destination)?;
    let next = inspect(new)?;
    let current_path = destination.canonicalize().map_err(|e| e.to_string())?;
    let next_path = new.canonicalize().map_err(|e| e.to_string())?;
    if current_path.starts_with(&next_path)
        || next_path.starts_with(&current_path)
        || backup == failed
    {
        return Err("Update and recovery paths must be separate".into());
    }
    if current.identifier != next.identifier || next.version <= current.version {
        return Err("Update identity mismatch or downgrade".into());
    }
    let parent = destination
        .parent()
        .ok_or("Bundle has no parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    for path in [backup, failed] {
        if path
            .parent()
            .ok_or("Recovery path has no parent")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            != parent
            || fs::symlink_metadata(path).is_ok()
        {
            return Err("Recovery paths must be new siblings of the app".into());
        }
    }
    fs::rename(destination, backup).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(new, destination) {
        return match fs::rename(backup, destination) {
            Ok(()) => Err(format!("Update failed; previous app restored: {error}")),
            Err(restore) => Err(format!(
                "Update failed: {error}; restore failed: {restore}; previous app retained at {}",
                backup.display()
            )),
        };
    }
    if let Err(error) = launch(destination) {
        if let Err(restore) =
            fs::rename(destination, failed).and_then(|_| fs::rename(backup, destination))
        {
            return Err(format!(
                "New app failed: {error}; restore failed: {restore}; previous app retained at {}",
                backup.display()
            ));
        }
        return Err(format!(
            "New app failed; previous app restored, failed bundle retained at {}: {error}",
            failed.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &Path, version: &str) {
        fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        fs::write(
            path.join("Contents/MacOS/rotor-desktop"),
            [0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0, 0, 1],
        )
        .unwrap();
        for relative in [
            "model/pp-ocrv6_tiny_det.onnx",
            "model/pp-ocrv6_tiny_rec.onnx",
            "model/ppocrv6_tiny_dict.txt",
        ] {
            let resource = path.join("Contents/Resources/assets").join(relative);
            fs::create_dir_all(resource.parent().unwrap()).unwrap();
            fs::write(resource, b"synthetic resource fixture").unwrap();
        }
        let value = plist::Value::Dictionary(plist::Dictionary::from_iter([
            ("CFBundleIdentifier", "cc.fluctus.rotor3.dev"),
            ("CFBundleExecutable", "rotor-desktop"),
            ("CFBundleShortVersionString", version),
        ]));
        value.to_file_xml(path.join("Contents/Info.plist")).unwrap();
    }
    #[test]
    fn launch_failure_restores_old_bundle_without_deleting_the_new_one() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("Rotor.app");
        let new = temp.path().join("New.app");
        let backup = temp.path().join("backup.app");
        let failed = temp.path().join("failed.app");
        fixture(&old, "2.6.0");
        fixture(&new, "2.7.0");
        assert!(replace_and_launch(&new, &old, &backup, &failed, |_| Err(
            "startup failed".into()
        ))
        .is_err());
        assert_eq!(inspect(&old).unwrap().version.to_string(), "2.6.0");
        assert_eq!(inspect(&failed).unwrap().version.to_string(), "2.7.0");
        assert!(replace_and_launch(&failed, &old, &backup, &failed, |_| Ok(())).is_err());
    }
    #[test]
    fn successful_replacement_retains_backup_and_rejects_downgrades() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("Rotor.app");
        let new = temp.path().join("New.app");
        let backup = temp.path().join("backup.app");
        let failed = temp.path().join("failed.app");
        fixture(&old, "2.6.0");
        fixture(&new, "2.7.0");
        replace_and_launch(&new, &old, &backup, &failed, |_| Ok(())).unwrap();
        assert_eq!(inspect(&old).unwrap().version.to_string(), "2.7.0");
        assert_eq!(inspect(&backup).unwrap().version.to_string(), "2.6.0");
        assert!(
            replace_and_launch(&backup, &old, &temp.path().join("b.app"), &failed, |_| Ok(
                ()
            ))
            .is_err()
        );
    }
    #[test]
    fn archive_links_are_rejected_before_following_them() {
        let mut compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        {
            let mut archive = tar::Builder::new(&mut compressed);
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_link_name("../../outside").unwrap();
            header.set_cksum();
            archive
                .append_data(&mut header, "Rotor.app/link", std::io::empty())
                .unwrap();
            archive.finish().unwrap();
        }
        let bytes = compressed.finish().unwrap();
        let temp = tempfile::tempdir().unwrap();
        assert!(extract(&bytes[..], temp.path())
            .unwrap_err()
            .contains("links"));
        assert!(!temp.path().join("Rotor.app/link").exists());
    }

    #[test]
    fn archive_traversal_and_duplicate_files_are_rejected() {
        for names in [
            vec!["../outside"],
            vec!["/absolute"],
            vec!["Rotor.app/../../outside"],
            vec!["Rotor.app/file", "Rotor.app/file"],
        ] {
            let mut compressed =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            {
                let mut archive = tar::Builder::new(&mut compressed);
                for name in names {
                    let mut header = tar::Header::new_gnu();
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_size(0);
                    header.set_mode(0o644);
                    header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
                    header.set_cksum();
                    archive.append(&header, std::io::empty()).unwrap();
                }
                archive.finish().unwrap();
            }
            let bytes = compressed.finish().unwrap();
            let temp = tempfile::tempdir().unwrap();
            assert!(extract(&bytes[..], temp.path()).is_err());
        }
    }

    #[test]
    fn ordinary_bundle_archive_extracts_with_its_identity() {
        let source = tempfile::tempdir().unwrap();
        let app = source.path().join("Rotor 3 Development.app");
        fixture(&app, "2.7.0");
        let mut compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        {
            let mut archive = tar::Builder::new(&mut compressed);
            archive
                .append_dir_all("Rotor 3 Development.app", &app)
                .unwrap();
            archive.finish().unwrap();
        }
        let bytes = compressed.finish().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let extracted = extract(&bytes[..], destination.path()).unwrap();
        assert_eq!(inspect(&extracted).unwrap().version.to_string(), "2.7.0");
    }

    #[test]
    fn preview_version_is_distinct_from_numeric_apple_bundle_version() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("Rotor.app");
        fixture(&app, "2.7.0");
        let path = app.join("Contents/Info.plist");
        let mut value = plist::Value::from_file(&path).unwrap();
        value
            .as_dictionary_mut()
            .unwrap()
            .insert("RotorVersion".into(), "2.7.0-beta.1".into());
        value.to_file_xml(path).unwrap();
        assert_eq!(inspect(&app).unwrap().version.to_string(), "2.7.0-beta.1");
    }

    #[test]
    fn production_bundle_uses_the_native_executable_name() {
        let root = tempfile::tempdir().unwrap();
        let app = root.path().join("Rotor.app");
        fixture(&app, "2.7.0");
        let path = app.join("Contents/Info.plist");
        let mut value = plist::Value::from_file(&path).unwrap();
        let dictionary = value.as_dictionary_mut().unwrap();
        dictionary.insert("CFBundleIdentifier".into(), "cc.fluctus.rotor3".into());
        dictionary.insert("CFBundleExecutable".into(), "rotor".into());
        value.to_file_xml(path).unwrap();
        fs::rename(
            app.join("Contents/MacOS/rotor-desktop"),
            app.join("Contents/MacOS/rotor"),
        )
        .unwrap();
        assert_eq!(
            inspect(&app).unwrap().executable.file_name().unwrap(),
            "rotor"
        );
    }
}
