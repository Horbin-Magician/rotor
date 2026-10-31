//! Developer ID signing is separate from Rotor's minisign updater signatures.
use crate::Result;
use std::{path::Path, process::Command};

pub struct Signing {
    identity: String,
    profile: String,
}

fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    if !command.status()?.success() {
        return Err(format!("{program} failed; do not distribute these packages").into());
    }
    Ok(())
}

impl Signing {
    pub fn from_env(production: bool) -> Result<Option<Self>> {
        Self::configured(
            production,
            std::env::var("APPLE_SIGNING_IDENTITY").ok(),
            std::env::var("APPLE_NOTARY_PROFILE").ok(),
        )
    }

    fn configured(
        production: bool,
        identity: Option<String>,
        profile: Option<String>,
    ) -> Result<Option<Self>> {
        match (identity, profile) {
            (None, None) if !production => Ok(None),
            (Some(identity), Some(profile))
                if identity.starts_with("Developer ID Application: ") && !profile.trim().is_empty() =>
            {
                Ok(Some(Self { identity, profile }))
            }
            _ => Err("macOS distribution requires APPLE_SIGNING_IDENTITY (Developer ID Application certificate name) and APPLE_NOTARY_PROFILE (notarytool Keychain profile)".into()),
        }
    }

    fn sign(&self, path: &Path, runtime: bool) -> Result<()> {
        let mut command = Command::new("codesign");
        command.args(["--force", "--timestamp", "--sign", &self.identity]);
        if runtime {
            command.args(["--options", "runtime"]);
        }
        run(command.arg(path))
    }

    fn notarize(&self, upload: &Path, staple: &Path) -> Result<()> {
        let mut command = Command::new("xcrun");
        command.args(["notarytool", "submit"]).arg(upload).args([
            "--keychain-profile",
            &self.profile,
            "--wait",
            "--output-format",
            "json",
        ]);
        if let Some(keychain) = std::env::var_os("APPLE_NOTARY_KEYCHAIN") {
            command.arg("--keychain").arg(keychain);
        }
        let result = command.output()?;
        let response: serde_json::Value = serde_json::from_slice(&result.stdout).map_err(|_| {
            format!(
                "notarytool returned no valid JSON: {}",
                String::from_utf8_lossy(&result.stderr)
            )
        })?;
        if !result.status.success() || response["status"] != "Accepted" {
            return Err(format!("Apple notarization did not accept the upload: {response}. Retrieve its log with xcrun notarytool log <id> --keychain-profile <profile>").into());
        }
        run(Command::new("xcrun")
            .args(["stapler", "staple"])
            .arg(staple))?;
        run(Command::new("xcrun")
            .args(["stapler", "validate"])
            .arg(staple))
    }

    pub fn app(&self, app: &Path) -> Result<()> {
        // Stage places the executable and all bundled dylibs directly in MacOS.
        // Sign dependencies first, then the enclosing bundle; never use --deep to sign.
        for entry in std::fs::read_dir(app.join("Contents/MacOS"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "dylib") {
                self.sign(&path, true)?;
            }
        }
        self.sign(app, true)?;
        run(Command::new("codesign")
            .args(["--verify", "--deep", "--strict", "--verbose=2"])
            .arg(app))?;
        let temporary = tempfile::tempdir()?;
        let zip = temporary.path().join("notarization.zip");
        run(Command::new("ditto")
            .args(["-c", "-k", "--keepParent"])
            .arg(app)
            .arg(&zip))?;
        self.notarize(&zip, app)?;
        run(Command::new("spctl")
            .args(["--assess", "--type", "execute", "--verbose=2"])
            .arg(app))
    }

    pub fn dmg(&self, dmg: &Path) -> Result<()> {
        self.sign(dmg, false)?;
        self.notarize(dmg, dmg)?;
        run(Command::new("codesign")
            .args(["--verify", "--strict"])
            .arg(dmg))?;
        run(Command::new("spctl")
            .args([
                "--assess",
                "--type",
                "open",
                "--context",
                "context:primary-signature",
                "--verbose=2",
            ])
            .arg(dmg))
    }
}

#[cfg(test)]
mod tests {
    use super::Signing;

    #[test]
    fn production_and_partial_configuration_cannot_fall_back_to_unsigned() {
        assert!(Signing::configured(false, None, None).unwrap().is_none());
        assert!(Signing::configured(true, None, None).is_err());
        assert!(
            Signing::configured(false, Some("Developer ID Application: Test".into()), None)
                .is_err()
        );
        assert!(Signing::configured(
            true,
            Some("Apple Development: Test".into()),
            Some("profile".into())
        )
        .is_err());
        assert!(Signing::configured(
            true,
            Some("Developer ID Application: Test".into()),
            Some("profile".into())
        )
        .unwrap()
        .is_some());
    }
}
