use crate::{version, Result};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn sign_with_key(
    path: &Path,
    secret_text: &str,
    password: &str,
    public_text: &str,
) -> Result<String> {
    let secret = minisign::SecretKeyBox::from_string(secret_text.trim())
        .map_err(|_| "Invalid signing key format")?
        .into_secret_key(Some(password.into()))
        .map_err(|_| "Cannot unlock signing key")?;
    let public = minisign::PublicKeyBox::from_string(public_text.trim())?.into_public_key()?;
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("Artifact name is not Unicode")?;
    if name.contains(['\r', '\n']) {
        return Err("Artifact name must be a single line".into());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let comment = format!("timestamp:{timestamp}\tfile:{name}");
    let signature = minisign::sign(
        Some(&public),
        &secret,
        fs::File::open(path)?,
        Some(&comment),
        Some("signature from Rotor update key"),
    )
    .map_err(|_| "Signing failed; check the native update key pair")?;
    let encoded = signature.to_string();
    rotor_updater::verify_file(path, &encoded, public_text)?;
    Ok(encoded)
}
fn signature_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".sig");
    PathBuf::from(name)
}
pub fn sign(path: &Path) -> Result<()> {
    let output = signature_path(path);
    if output.exists() {
        rotor_updater::verify_file(
            path,
            &fs::read_to_string(&output)?,
            rotor_updater::PUBLIC_KEY,
        )?;
        println!("Existing signature verified: {}", output.display());
        return Ok(());
    }
    let secret = std::env::var("ROTOR_SIGNING_PRIVATE_KEY")
        .map_err(|_| "Set ROTOR_SIGNING_PRIVATE_KEY to the minisign key text or key file path")?;
    let secret = if Path::new(&secret).is_file() {
        fs::read_to_string(&secret)?
    } else {
        secret
    };
    let password = std::env::var("ROTOR_SIGNING_PRIVATE_KEY_PASSWORD").unwrap_or_default();
    let signature = sign_with_key(path, &secret, &password, rotor_updater::PUBLIC_KEY)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    writeln!(file, "{signature}")?;
    file.sync_all()?;
    println!("Signed and verified: {}", output.display());
    Ok(())
}
fn build_manifest(
    directory: &Path,
    base: &str,
    notes: &str,
    version: &str,
    public_key: &str,
    production: bool,
) -> Result<rotor_updater::Manifest> {
    semver::Version::parse(version)?;
    let mut base = url::Url::parse(base)?;
    if base.scheme() != "https"
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(
            "Release asset base must be an HTTPS directory URL without credentials/query/fragment"
                .into(),
        );
    }
    if !base.path().ends_with('/') {
        base.set_path(&format!("{}/", base.path()));
    }
    let mut platforms = HashMap::new();
    for (suffix, aliases) in [
        ("x64-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
        (
            "aarch64.app.tar.gz",
            ["darwin-aarch64", "darwin-aarch64-app"],
        ),
    ] {
        let name = if production {
            if suffix == "aarch64.app.tar.gz" {
                "Rotor_aarch64.app.tar.gz".into()
            } else {
                format!("Rotor_{version}_{suffix}")
            }
        } else {
            format!("Rotor-GPUI_{version}_{suffix}")
        };
        let artifact = directory.join(&name);
        let signature = fs::read_to_string(signature_path(&artifact))?;
        rotor_updater::verify_file(&artifact, &signature, public_key)?;
        let artifact = rotor_updater::Artifact {
            signature: signature.trim().into(),
            url: base.join(&name)?.to_string(),
        };
        for alias in aliases {
            platforms.insert(alias.into(), artifact.clone());
        }
    }
    Ok(rotor_updater::Manifest {
        schema_version: 1,
        version: version.into(),
        notes: notes.into(),
        pub_date: None,
        platforms,
    })
}
pub fn manifest(directory: &Path, base: &str, notes: &Path, output: &Path) -> Result<()> {
    let info = crate::builder::staged_info(directory)?;
    let manifest = build_manifest(
        directory,
        base,
        &fs::read_to_string(notes)?,
        &version()?,
        rotor_updater::PUBLIC_KEY,
        info.production,
    )?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
    file.sync_all()?;
    println!("Verified two-platform manifest: {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_requires_both_signed_platforms_and_preserves_notes() {
        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let public = pair.pk.to_box().unwrap().to_string();
        let root = tempfile::tempdir().unwrap();
        for suffix in ["x64-setup.exe", "aarch64.app.tar.gz"] {
            let path = root.path().join(format!("Rotor-GPUI_2.7.0_{suffix}"));
            fs::write(&path, b"synthetic artifact").unwrap();
            let signature = minisign::sign(
                Some(&pair.pk),
                &pair.sk,
                fs::File::open(&path).unwrap(),
                None,
                None,
            )
            .unwrap();
            fs::write(signature_path(&path), signature.to_string()).unwrap();
        }
        let notes = "line one\n\"quoted\" release";
        let manifest = build_manifest(
            root.path(),
            "https://example.com/releases/gpui-latest",
            notes,
            "2.7.0",
            &public,
            false,
        )
        .unwrap();
        assert_eq!(manifest.notes, notes);
        assert_eq!(manifest.platforms.len(), 4);
        assert!(manifest.platforms["windows-x86_64"]
            .url
            .contains("/gpui-latest/Rotor-GPUI_2.7.0_"));
        assert!(build_manifest(
            root.path(),
            "http://example.com/",
            notes,
            "2.7.0",
            &public,
            false
        )
        .is_err());
        fs::write(
            root.path().join("Rotor-GPUI_2.7.0_aarch64.app.tar.gz"),
            b"tampered",
        )
        .unwrap();
        assert!(build_manifest(
            root.path(),
            "https://example.com/",
            notes,
            "2.7.0",
            &public,
            false
        )
        .is_err());
    }
    #[test]
    fn encrypted_signing_keys_produce_native_signatures() {
        let pair =
            minisign::KeyPair::generate_encrypted_keypair(Some("fixture password".into())).unwrap();
        let public = pair.pk.to_box().unwrap().to_string();
        let secret = pair.sk.to_box(None).unwrap().to_string();
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(file.path(), b"fixture artifact").unwrap();
        let signature = sign_with_key(file.path(), &secret, "fixture password", &public).unwrap();
        rotor_updater::verify_file(file.path(), &signature, &public).unwrap();
        assert!(sign_with_key(file.path(), &secret, "wrong password", &public).is_err());
        assert!(sign_with_key(
            file.path(),
            &secret,
            "fixture password",
            rotor_updater::PUBLIC_KEY
        )
        .is_err());
        fs::write(file.path(), b"tampered").unwrap();
        assert!(rotor_updater::verify_file(file.path(), &signature, &public).is_err());
    }
}
