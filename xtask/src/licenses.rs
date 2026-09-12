use crate::{root, Result};
use std::{collections::HashSet, fs, path::Path, process::Command};

/// Retain original license/notice files from the resolved dependency sources.
pub fn stage(destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    crate::copy_tree(
        &root().join("native/licenses"),
        &destination.join("runtimes"),
    )?;
    for (source, name) in [
        ("LICENSE", "Rotor-LICENSE"),
        ("THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"),
        (
            "crates/rotor-ui/src/search_icons/LICENSE",
            "Material-Icons-LICENSE",
        ),
        (
            "native/gpui-platform/LICENSE-APACHE",
            "GPUI-Platform-LICENSE",
        ),
        ("native/gpui-macos/LICENSE-APACHE", "GPUI-macOS-LICENSE"),
    ] {
        fs::copy(root().join(source), destination.join(name))?;
    }
    let target = if cfg!(windows) {
        "x86_64-pc-windows-msvc"
    } else {
        "aarch64-apple-darwin"
    };
    let output = Command::new("cargo")
        .current_dir(root())
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            target,
        ])
        .output()?;
    if !output.status.success() {
        return Err("Cannot resolve dependency license inventory".into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let resolved: HashSet<_> = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("Missing dependency graph")?
        .iter()
        .filter_map(|node| node["id"].as_str())
        .collect();
    let mut inventory = Vec::new();
    for package in metadata["packages"].as_array().ok_or("Missing packages")? {
        if package["source"].is_null()
            || !resolved.contains(package["id"].as_str().ok_or("Missing package ID")?)
        {
            continue;
        }
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("Missing package path")?,
        );
        let source = manifest.parent().ok_or("Missing package directory")?;
        let name = format!(
            "{}-{}",
            package["name"].as_str().ok_or("Missing package name")?,
            package["version"]
                .as_str()
                .ok_or("Missing package version")?
        );
        let output = destination.join("dependencies").join(&name);
        fs::create_dir_all(&output)?;
        let mut files = Vec::new();
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let filename = entry.file_name().to_string_lossy().into_owned();
            let upper = filename.to_ascii_uppercase();
            if entry.file_type()?.is_file()
                && ["LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT"]
                    .iter()
                    .any(|prefix| upper.starts_with(prefix))
            {
                fs::copy(entry.path(), output.join(&filename))?;
                files.push(filename);
            }
        }
        if let Some(path) = package["license_file"].as_str() {
            let path = source.join(path);
            if path.is_file() {
                fs::copy(path, output.join("DECLARED-LICENSE"))?;
                files.push("DECLARED-LICENSE".into());
            }
        }
        files.sort();
        inventory.push(serde_json::json!({
            "package": name, "license": package["license"], "repository": package["repository"],
            "source": package["source"], "files": files,
        }));
    }
    fs::write(
        destination.join("dependencies.json"),
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    Ok(())
}
