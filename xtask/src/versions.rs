use crate::{root, version, Result};
use std::{fs, process::Command};

fn updated(cargo: &str, next: &str) -> Result<String> {
    let next = semver::Version::parse(next)?.to_string();
    let mut cargo: toml_edit::DocumentMut = cargo.parse()?;
    if !cargo["workspace"]["package"]["version"].is_str() {
        return Err("Workspace version is missing".into());
    }
    cargo["workspace"]["package"]["version"] = toml_edit::value(next);
    Ok(cargo.to_string())
}
pub fn set(next: &str, dry_run: bool) -> Result<()> {
    let previous = version()?;
    let cargo = root().join("Cargo.toml");
    let lock = root().join("Cargo.lock");
    let original_cargo = fs::read_to_string(&cargo)?;
    let original_lock = fs::read(&lock)?;
    let next_cargo = updated(&original_cargo, next)?;
    if dry_run {
        println!("Would update workspace version {previous} -> {next} and refresh Cargo.lock. No commit, tag or push.");
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
    fn version_edit_preserves_workspace_settings_without_a_frontend_manifest() {
        let cargo = super::updated(
            "# preserved\n[workspace.package]\nversion = '2.6.0'\n[workspace]\nresolver = '2'\n",
            "2.7.0-beta.1",
        )
        .unwrap();
        assert!(cargo.contains("# preserved"));
        assert!(cargo.contains("resolver = '2'"));
        assert!(cargo.contains("2.7.0-beta.1"));
        assert!(super::updated(&cargo, "invalid").is_err());
    }
}
