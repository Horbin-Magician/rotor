use crate::{root, verify_stage, version, write_manifest, Result};
use rotor_common::native_app::BuildInfo;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn flavor(production: bool) -> &'static str {
    if production {
        "production"
    } else {
        "development"
    }
}
fn snapshot_root() -> PathBuf {
    std::env::var_os("RUNNER_TEMP")
        .map(PathBuf::from)
        .map(|path| path.join("rotor-native-binaries"))
        .unwrap_or_else(|| root().join("target/native-binaries"))
}
fn source_digest() -> Result<String> {
    let listed = Command::new("git")
        .current_dir(root())
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates",
            "assets",
            "native",
            "xtask",
            ".cargo",
            ".gitattributes",
        ])
        .output()?;
    if !listed.status.success() {
        return Err("Cannot enumerate source inputs for native provenance".into());
    }
    let mut names = listed
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8(name.to_vec()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    names.sort();
    names.dedup();
    let mut digest = Sha256::new();
    for name in names {
        let relative = Path::new(&name);
        if relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Invalid source input path".into());
        }
        let path = root().join(relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.file_type().is_symlink() {
            return Err("Native provenance does not accept source symlinks".into());
        }
        let mut bytes = fs::read(&path)?;
        if matches!(
            path.extension().and_then(|s| s.to_str()),
            Some("rs" | "toml" | "lock" | "json" | "md" | "txt" | "nsi" | "xml" | "key")
        ) {
            bytes = String::from_utf8(bytes)?.replace("\r\n", "\n").into_bytes();
        }
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn binary_name() -> &'static str {
    if cfg!(windows) {
        "rotor-desktop.exe"
    } else {
        "rotor-desktop"
    }
}
pub fn supported_host() -> Result<()> {
    if !matches!(
        (std::env::consts::OS, std::env::consts::ARCH),
        ("windows", "x86_64") | ("macos", "aarch64")
    ) {
        return Err("Native packaging requires Windows x64 or macOS arm64".into());
    }
    Ok(())
}
pub fn build(production: bool, options: &[String]) -> Result<()> {
    supported_host()?;
    let version = version()?;
    let source = source_digest()?;
    if options.iter().any(|option| {
        ["--target", "--target-dir", "--profile"]
            .iter()
            .any(|prefix| option.starts_with(prefix))
    }) {
        return Err(
            "xtask snapshots native host release builds; target/profile overrides are unsupported"
                .into(),
        );
    }
    let mut command = Command::new("cargo");
    command.current_dir(root()).args([
        "build",
        "-p",
        "rotor-desktop",
        "--release",
        "--locked",
        "--no-default-features",
    ]);
    // Snapshot the output we actually built even if the caller has a different
    // CARGO_TARGET_DIR in its environment or Cargo configuration.
    command.arg("--target-dir").arg(root().join("target"));
    if production {
        command.args(["--features", "production"]);
    }
    let status = command.args(options).status()?;
    if !status.success() {
        return Err("Native build failed".into());
    }
    let release = root().join("target/release");
    // Only interrogate the executable immediately built from this source, whose
    // --build-info path returns before data directories, windows or services.
    let result = Command::new(release.join(binary_name()))
        .arg("--build-info")
        .output()?;
    if !result.status.success() {
        return Err("Native build information probe failed".into());
    }
    let info: BuildInfo = serde_json::from_slice(&result.stdout)?;
    let mut expected = rotor_common::native_app::build_info_for(production);
    expected.version = version.clone();
    if info != expected {
        return Err(
            "Built binary has a different flavor/version; use --production for production identity"
                .into(),
        );
    }
    if source_digest()? != source {
        return Err("Source inputs changed during build; rebuild before staging".into());
    }
    fs::create_dir_all(snapshot_root())?;
    let snapshot = tempfile::Builder::new()
        .prefix(&format!("{}-", flavor(production)))
        .tempdir_in(snapshot_root())?
        .keep();
    fs::copy(release.join(binary_name()), snapshot.join(binary_name()))?;
    if cfg!(windows) {
        fs::copy(
            release.join("rotor-recovery.exe"),
            snapshot.join("rotor-recovery.exe"),
        )?;
    }
    for entry in fs::read_dir(&release)? {
        let entry = entry?;
        if matches!(
            entry.path().extension().and_then(|s| s.to_str()),
            Some("dll" | "dylib")
        ) {
            fs::copy(
                entry.path().canonicalize()?,
                snapshot.join(entry.file_name()),
            )?;
        }
    }
    fs::write(
        snapshot.join("native-build.json"),
        serde_json::to_vec_pretty(&info)?,
    )?;
    fs::write(snapshot.join("source.sha256"), &source)?;
    write_manifest(&snapshot, &version)?;
    rotor_common::persistence::atomic_write_private(
        &snapshot_root().join(format!("{}.json", flavor(production))),
        &serde_json::to_vec(&snapshot)?,
    )?;
    println!(
        "Verified {} binary snapshot: {}",
        flavor(production),
        snapshot.display()
    );
    Ok(())
}
pub fn snapshot(production: bool) -> Result<(PathBuf, BuildInfo)> {
    supported_host()?;
    let path: PathBuf = serde_json::from_slice(
        &fs::read(snapshot_root().join(format!("{}.json", flavor(production))))
            .map_err(|_| "Build this flavor with xtask before staging")?,
    )?;
    let path = path.canonicalize()?;
    if !path.starts_with(snapshot_root().canonicalize()?) {
        return Err("Binary snapshot escaped the build directory".into());
    }
    verify_stage(&path)?;
    verify_source(&path)?;
    let info: BuildInfo = serde_json::from_slice(&fs::read(path.join("native-build.json"))?)?;
    validate(&info, production)?;
    Ok((path, info))
}
pub fn validate(info: &BuildInfo, production: bool) -> Result<()> {
    let mut expected = rotor_common::native_app::build_info_for(production);
    expected.version = version()?;
    if info != &expected {
        return Err("Binary/staging flavor or version differs; rebuild the intended flavor".into());
    }
    Ok(())
}
pub fn staged_info(directory: &Path) -> Result<BuildInfo> {
    verify_source(directory)?;
    let info: BuildInfo = serde_json::from_slice(&fs::read(directory.join("native-build.json"))?)?;
    validate(&info, info.production)?;
    Ok(info)
}
fn verify_source(directory: &Path) -> Result<()> {
    let previous = fs::read_to_string(directory.join("source.sha256"))
        .map_err(|_| "Build/stage again to create source provenance")?;
    if previous != source_digest()? {
        return Err(
            "Native source/assets changed since the snapshot; rebuild and stage again".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_receipts_reject_stale_snapshot_inputs() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("source.sha256"), "stale").unwrap();
        assert!(super::verify_source(directory.path()).is_err());
        let digest = super::source_digest().unwrap();
        assert_eq!(digest.len(), 64);
        std::fs::write(directory.path().join("source.sha256"), digest).unwrap();
        super::verify_source(directory.path()).unwrap();
    }
    #[test]
    fn staged_identity_cannot_be_relabelled_as_the_other_flavor() {
        let mut info = rotor_common::native_app::build_info_for(false);
        info.version = crate::version().unwrap();
        super::validate(&info, false).unwrap();
        assert!(super::validate(&info, true).is_err());
        info.version = "0.0.0".into();
        assert!(super::validate(&info, false).is_err());
    }
}
