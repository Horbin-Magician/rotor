use crate::{root, version, Result};
use std::{fs, process::Command};

fn updated(cargo: &str, package: &str, next: &str) -> Result<(String, String)> {
    let next = semver::Version::parse(next)?.to_string();
    let mut cargo: toml_edit::DocumentMut = cargo.parse()?;
    let mut package: serde_json::Value = serde_json::from_str(package)?;
    if !cargo["workspace"]["package"]["version"].is_str() || !package["version"].is_string() {
        return Err("Release version fields are missing".into());
    }
    cargo["workspace"]["package"]["version"] = toml_edit::value(next.clone());
    package["version"] = next.into();
    Ok((
        cargo.to_string(),
        format!("{}\n", serde_json::to_string_pretty(&package)?),
    ))
}
pub fn set(next: &str, dry_run: bool) -> Result<()> {
    let previous = version()?;
    let cargo = root().join("Cargo.toml");
    let package = root().join("package.json");
    let lock = root().join("Cargo.lock");
    let original_cargo = fs::read_to_string(&cargo)?;
    let original_package = fs::read_to_string(&package)?;
    let original_lock = fs::read(&lock)?;
    let (next_cargo, next_package) = updated(&original_cargo, &original_package, next)?;
    if dry_run {
        println!("Would update workspace/package versions {previous} -> {next} and refresh Cargo.lock. No commit, tag or push.");
        return Ok(());
    }
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root())
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err("Version editing requires a clean working tree".into());
    }
    let result = (|| -> Result<()> {
        fs::write(&cargo, next_cargo)?;
        fs::write(&package, next_package)?;
        let status = Command::new("cargo")
            .args(["update", "--workspace", "--offline"])
            .current_dir(root())
            .status()?;
        if !status.success() {
            return Err("Could not refresh workspace lockfile".into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        let restored = [
            fs::write(&cargo, original_cargo),
            fs::write(&package, original_package),
            fs::write(&lock, original_lock),
        ];
        if restored.iter().any(|result| result.is_err()) {
            return Err(format!(
                "{error}; restoring version files failed, recover them from Git before continuing"
            )
            .into());
        }
        return Err(error);
    }
    println!("Updated to {next}. Review and commit/tag/push explicitly when ready.");
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn version_edit_preserves_workspace_settings_and_package_order() {
        let (cargo, package) = super::updated(
            "# preserved\n[workspace.package]\nversion = '2.6.0'\n[workspace]\nresolver = '2'\n",
            "{\"name\":\"rotor\",\"version\":\"2.6.0\",\"scripts\":{\"test\":\"keep\"}}",
            "2.7.0-beta.1",
        )
        .unwrap();
        assert!(cargo.contains("# preserved"));
        assert!(cargo.contains("resolver = '2'"));
        assert!(package.find("name").unwrap() < package.find("version").unwrap());
        assert!(package.contains("2.7.0-beta.1"));
        assert!(package.contains("keep"));
        assert!(super::updated(&cargo, &package, "invalid").is_err());
    }
}
