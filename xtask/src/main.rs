mod builder;
mod installer;
mod release;
mod versions;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn version() -> Result<String> {
    let cargo: toml::Value = fs::read_to_string(root().join("Cargo.toml"))?.parse()?;
    let version = cargo["workspace"]["package"]["version"]
        .as_str()
        .ok_or("missing workspace version")?
        .to_string();
    semver::Version::parse(&version)?;
    Ok(version)
}
fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn files(directory: &Path, result: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            return Err(format!("unexpected staged symlink: {}", entry.path().display()).into());
        }
        if entry.file_type()?.is_dir() {
            files(&entry.path(), result)?;
        } else {
            result.push(entry.path());
        }
    }
    result.sort();
    Ok(())
}
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            return Err("resource symlinks are not allowed".into());
        }
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
fn write_manifest(directory: &Path, version: &str) -> Result<()> {
    let mut paths = Vec::new();
    files(directory, &mut paths)?;
    let mut entries = serde_json::Map::new();
    for path in paths {
        if path == directory.join("resources.json") {
            continue;
        }
        entries.insert(
            path.strip_prefix(directory)?
                .to_string_lossy()
                .replace('\\', "/"),
            serde_json::json!({"bytes": fs::metadata(&path)?.len(), "sha256": hash(&path)?}),
        );
    }
    fs::write(
        directory.join("resources.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"version": version, "files": entries}))?,
    )?;
    Ok(())
}
fn verify_stage(directory: &Path) -> Result<()> {
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("resources.json"))?)?;
    let expected = manifest["files"]
        .as_object()
        .ok_or("invalid file manifest")?;
    let mut paths = Vec::new();
    files(directory, &mut paths)?;
    paths.retain(|path| path != &directory.join("resources.json"));
    if paths.len() != expected.len() {
        return Err("staged file set changed".into());
    }
    for path in paths {
        let name = path
            .strip_prefix(directory)?
            .to_string_lossy()
            .replace('\\', "/");
        let entry = expected.get(&name).ok_or("unexpected staged file")?;
        if entry["sha256"].as_str() != Some(hash(&path)?.as_str())
            || entry["bytes"].as_u64() != Some(fs::metadata(path)?.len())
        {
            return Err(format!("staged checksum mismatch: {name}").into());
        }
    }
    Ok(())
}
fn stage(directory: &Path, production: bool) -> Result<()> {
    let version = version()?;
    let (snapshot, info) = builder::snapshot(production)?;
    let parsed = semver::Version::parse(&version)?;
    let mac_version = format!("{}.{}.{}", parsed.major, parsed.minor, parsed.patch);
    let mut config: toml::Value = fs::read_to_string(root().join("native/app.toml"))?.parse()?;
    config["product_name"] = info.product_name.clone().into();
    config["identifier"] = info.identifier.clone().into();
    config["data_directory"] = info.profile_directory.clone().into();
    if production {
        config["update_endpoints"] = config["production_update_endpoints"].clone();
        config["update_channel"] = "gpui-preview-production".into();
    }
    config
        .as_table_mut()
        .unwrap()
        .insert("production".into(), production.into());
    fs::create_dir(directory)?;
    let (executable_dir, resource_dir) = if cfg!(target_os = "macos") {
        let contents = directory.join(format!("{}.app/Contents", info.product_name));
        let executable_dir = contents.join("MacOS");
        let resource_dir = contents.join("Resources");
        fs::create_dir_all(&executable_dir)?;
        fs::create_dir_all(&resource_dir)?;
        let mut dictionary = plist::Dictionary::new();
        for (key, value) in [
            ("CFBundleExecutable", info.executable_name.as_str()),
            ("CFBundleIdentifier", info.identifier.as_str()),
            ("CFBundleName", info.product_name.as_str()),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleShortVersionString", &mac_version),
            ("CFBundleVersion", &mac_version),
            ("RotorVersion", &version),
            ("CFBundleIconFile", "icon.icns"),
            (
                "LSMinimumSystemVersion",
                config["minimum_macos"]
                    .as_str()
                    .ok_or("missing minimum_macos")?,
            ),
        ] {
            dictionary.insert(key.into(), plist::Value::String(value.into()));
        }
        dictionary.insert("LSUIElement".into(), plist::Value::Boolean(true));
        dictionary.insert(
            "NSHighResolutionCapable".into(),
            plist::Value::Boolean(true),
        );
        plist::Value::Dictionary(dictionary).to_file_xml(contents.join("Info.plist"))?;
        fs::copy(
            root().join("assets/icons/icon.icns"),
            resource_dir.join("icon.icns"),
        )?;
        (executable_dir, resource_dir)
    } else {
        (directory.to_path_buf(), directory.to_path_buf())
    };
    let binary = format!(
        "{}{}",
        info.executable_name,
        if cfg!(windows) { ".exe" } else { "" }
    );
    fs::copy(
        snapshot.join(builder::binary_name()),
        executable_dir.join(binary),
    )?;
    if cfg!(windows) {
        fs::copy(
            snapshot.join("rotor-recovery.exe"),
            executable_dir.join("rotor-recovery.exe"),
        )?;
    }
    for entry in fs::read_dir(&snapshot)? {
        let entry = entry?;
        if matches!(
            entry.path().extension().and_then(|s| s.to_str()),
            Some("dll" | "dylib")
        ) {
            fs::copy(entry.path(), executable_dir.join(entry.file_name()))?;
        }
    }
    copy_tree(&root().join("assets"), &resource_dir.join("assets"))?;
    fs::write(
        resource_dir.join("native-app.toml"),
        toml::to_string_pretty(&config)?,
    )?;
    fs::copy(
        root().join("native/update-public.key"),
        resource_dir.join("update-public.key"),
    )?;
    fs::write(
        directory.join("native-build.json"),
        serde_json::to_vec_pretty(&info)?,
    )?;
    fs::copy(
        snapshot.join("source.sha256"),
        directory.join("source.sha256"),
    )?;
    write_manifest(directory, &version)?;
    verify_stage(directory)?;
    println!(
        "Staged {} {} at {}",
        info.product_name,
        version,
        directory.display()
    );
    Ok(())
}
fn package(directory: &Path, output: &Path) -> Result<()> {
    builder::supported_host()?;
    verify_stage(directory)?;
    let info = builder::staged_info(directory)?;
    let version = version()?;
    let prefix = if info.production {
        "Rotor"
    } else {
        "Rotor-GPUI"
    };
    fs::create_dir(output)?;
    let output = output.canonicalize()?;
    let directory = directory.canonicalize()?;
    if cfg!(windows) {
        use std::io::Write;
        let executable = format!("{}.exe", info.executable_name);
        let mut uninstall = tempfile::Builder::new().suffix(".nsh").tempfile()?;
        uninstall.write_all(installer::uninstall_script(&directory, &executable)?.as_bytes())?;
        uninstall.flush()?;
        let parsed = semver::Version::parse(&version)?;
        let registry = if info.production {
            "Rotor"
        } else {
            "RotorGpuiDevelopment"
        };
        let compiler = std::env::var_os("NSIS_MAKENSIS").unwrap_or_else(|| "makensis.exe".into());
        let status = Command::new(compiler)
            .arg(format!("/DSTAGE_DIR={}", directory.display()))
            .arg(format!(
                "/DOUTPUT_FILE={}",
                output
                    .join(format!("{prefix}_{version}_x64-setup.exe"))
                    .display()
            ))
            .arg(format!("/DAPP_VERSION={version}"))
            .arg(format!(
                "/DNUMERIC_VERSION={}.{}.{}.0",
                parsed.major, parsed.minor, parsed.patch
            ))
            .arg(format!("/DPRODUCT_NAME={}", info.product_name))
            .arg(format!("/DAPP_EXE={executable}"))
            .arg(format!("/DREGISTRY_KEY={registry}"))
            .arg(format!("/DLEGACY_MUTEX={}-sim", info.identifier))
            .arg(format!(
                "/DUNINSTALL_INCLUDE={}",
                uninstall.path().display()
            ))
            .arg(root().join("native/windows.nsi"))
            .status()?;
        if !status.success() {
            return Err("NSIS packaging failed".into());
        }
    } else if cfg!(target_os = "macos") {
        let app_name = format!("{}.app", info.product_name);
        let archive_name = if info.production {
            "Rotor_aarch64.app.tar.gz".into()
        } else {
            format!("{prefix}_{version}_aarch64.app.tar.gz")
        };
        let status = Command::new("tar")
            .env("COPYFILE_DISABLE", "1")
            .arg("-czf")
            .arg(output.join(archive_name))
            .arg("-C")
            .arg(&directory)
            .arg(&app_name)
            .status()?;
        if !status.success() {
            return Err("macOS archive failed".into());
        }
        let dmg_root = tempfile::Builder::new().prefix("rotor-dmg-").tempdir()?;
        copy_tree(&directory.join(&app_name), &dmg_root.path().join(&app_name))?;
        #[cfg(target_os = "macos")]
        std::os::unix::fs::symlink("/Applications", dmg_root.path().join("Applications"))?;
        let status = Command::new("hdiutil")
            .args(["create", "-format", "UDZO", "-volname"])
            .arg(&info.product_name)
            .arg("-srcfolder")
            .arg(dmg_root.path())
            .arg(output.join(format!("{prefix}_{version}_aarch64.dmg")))
            .status()?;
        if !status.success() {
            return Err("DMG creation failed".into());
        }
    }
    fs::write(
        output.join("native-build.json"),
        serde_json::to_vec_pretty(&info)?,
    )?;
    fs::copy(
        directory.join("source.sha256"),
        output.join("source.sha256"),
    )?;
    write_manifest(&output, &version)?;
    println!("Packaged {} at {}", info.product_name, output.display());
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("import-profile") if args.len() == 5 => {
            let receipt = rotor_common::profile_migration::import(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]), &args[4])?;
            println!("Imported {} files. Original and backup retained; indexes will rebuild.", receipt.files.len());
        }
        Some("verify-profile") if args.len() == 2 => {
            let receipt = rotor_common::profile_migration::verify(Path::new(&args[1]))?;
            println!("Verified {} profile files from {}", receipt.files.len(), receipt.source_version);
        }
        Some("set-version") if args.len() == 2 || (args.len() == 3 && args[2] == "--dry-run") => versions::set(&args[1], args.len() == 3)?,
        Some("sign") if args.len() == 2 => release::sign(Path::new(&args[1]))?,
        Some("release-manifest") if args.len() == 5 => release::manifest(Path::new(&args[1]), &args[2], Path::new(&args[3]), Path::new(&args[4]))?,
        Some("inventory") if args.len() == 2 => write_manifest(Path::new(&args[1]), &version()?)?,
        Some("version") => println!("{}", version()?),
        Some("build") => {
            let production = args[1..].iter().any(|arg| arg == "--production");
            let options = args[1..].iter().filter(|arg| arg.as_str() != "--production").cloned().collect::<Vec<_>>();
            builder::build(production, &options)?;
        }
        Some("package") if args.len() == 3 => package(Path::new(&args[1]), Path::new(&args[2]))?,
        Some("stage") if args.len() == 2 => stage(Path::new(&args[1]), false)?,
        Some("stage") if args.len() == 3 && args[1] == "--production" => stage(Path::new(&args[2]), true)?,
        Some("verify") if args.len() == 2 => verify_stage(Path::new(&args[1]))?,
        _ => return Err("usage: cargo run -p xtask -- version | build [--production] [cargo options] | stage [--production] <new directory> | verify <directory> | package <stage directory> <new output directory> | import-profile <source> <new destination> <new backup> <source version> | verify-profile <directory> | inventory <directory> | set-version <semver> [--dry-run] | sign <artifact> | release-manifest <artifacts> <https base> <notes file> <new output>".into()),
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifests_detect_modified_missing_and_extra_resources() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("model.onnx");
        fs::write(&path, b"model").unwrap();
        write_manifest(directory.path(), "2.6.0").unwrap();
        verify_stage(directory.path()).unwrap();
        fs::write(&path, b"wrong").unwrap();
        assert!(verify_stage(directory.path()).is_err());
        fs::write(&path, b"model").unwrap();
        fs::write(directory.path().join("extra"), b"extra").unwrap();
        assert!(verify_stage(directory.path()).is_err());
        write_manifest(directory.path(), "2.6.0").unwrap();
        verify_stage(directory.path()).unwrap();
        fs::remove_file(directory.path().join("extra")).unwrap();
        fs::remove_file(path).unwrap();
        assert!(verify_stage(directory.path()).is_err());
    }
}
