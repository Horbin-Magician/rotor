pub mod bundle;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{handoff_error, launch_handoff, run_helper};

use base64::prelude::*;
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;
pub use tokio_util::sync::CancellationToken;

pub const PUBLIC_KEY: &str = include_str!("../../../native/update-public.key");
pub const PREVIEW_ENDPOINTS: &[&str] = &[
    "https://gitee.com/horbin/rotor/releases/download/gpui-latest/gpui-latest.json",
    "https://github.com/Horbin-Magician/rotor/releases/download/gpui-latest/gpui-latest.json",
];
pub const PRODUCTION_PREVIEW_ENDPOINTS: &[&str] = &[
    "https://gitee.com/horbin/rotor/releases/download/gpui-latest/gpui-production-latest.json",
    "https://github.com/Horbin-Magician/rotor/releases/download/gpui-latest/gpui-production-latest.json",
];
const MAX_MANIFEST: usize = 1024 * 1024;
const MAX_DOWNLOAD: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Artifact {
    pub signature: String,
    pub url: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub pub_date: Option<String>,
    pub platforms: HashMap<String, Artifact>,
}
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub artifact: Artifact,
    pub target: String,
}
#[derive(Clone, Debug)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub fn select_release(
    manifest: Manifest,
    current: &str,
    target: &str,
) -> Result<Option<Release>, String> {
    let version = semver::Version::parse(manifest.version.trim_start_matches('v'))
        .map_err(|error| error.to_string())?;
    let current = semver::Version::parse(current).map_err(|error| error.to_string())?;
    if version <= current {
        return Ok(None);
    }
    let artifact = manifest
        .platforms
        .get(target)
        .ok_or_else(|| format!("Update does not support {target}"))?
        .clone();
    let url = url::Url::parse(&artifact.url).map_err(|error| error.to_string())?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err("Update artifacts require HTTPS".into());
    }
    if artifact.signature.trim().is_empty() {
        return Err("Update artifact has no signature".into());
    }
    Ok(Some(Release {
        version: version.to_string(),
        notes: manifest.notes,
        artifact,
        target: target.into(),
    }))
}

pub async fn check(
    endpoints: &[String],
    current: &str,
    target: &str,
) -> Result<Option<Release>, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| error.to_string())?;
    if endpoints.is_empty() {
        return Err("No update endpoints configured".into());
    }
    let mut errors = Vec::new();
    for endpoint in endpoints {
        let request = async {
            let url = url::Url::parse(endpoint).map_err(|error| error.to_string())?;
            if url.scheme() != "https" {
                return Err("Update metadata requires HTTPS".into());
            }
            let response = client
                .get(url)
                .send()
                .await
                .map_err(|error| error.without_url().to_string())?
                .error_for_status()
                .map_err(|error| error.without_url().to_string())?;
            let mut stream = response.bytes_stream();
            let mut bytes = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                if bytes.len() + chunk.len() > MAX_MANIFEST {
                    return Err("Update metadata is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let manifest = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            select_release(manifest, current, target)
        }
        .await;
        match request {
            Ok(release) => return Ok(release),
            Err(error) => errors.push(error),
        }
    }
    Err(errors.join("; "))
}

pub fn verify(bytes: &[u8], signature: &str, public_key: &str) -> Result<(), String> {
    let (key, signature) = decode_signature(signature, public_key)?;
    key.verify(bytes, &signature, true)
        .map_err(|error| error.to_string())
}

fn decode_signature(signature: &str, public_key: &str) -> Result<(PublicKey, Signature), String> {
    let key = BASE64_STANDARD
        .decode(public_key.trim())
        .map_err(|error| error.to_string())?;
    let key = std::str::from_utf8(&key).map_err(|error| error.to_string())?;
    let key = PublicKey::decode(key).map_err(|error| error.to_string())?;
    let signature = BASE64_STANDARD
        .decode(signature.trim())
        .map_err(|error| error.to_string())?;
    let signature = std::str::from_utf8(&signature).map_err(|error| error.to_string())?;
    let signature = Signature::decode(signature).map_err(|error| error.to_string())?;
    Ok((key, signature))
}

pub fn verify_file(path: &Path, signature: &str, public_key: &str) -> Result<(), String> {
    use std::io::Read;
    let (key, signature) = decode_signature(signature, public_key)?;
    match key.verify_stream(&signature) {
        Ok(mut verifier) => {
            let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
                if count == 0 {
                    break;
                }
                verifier.update(&buffer[..count]);
            }
            verifier.finalize().map_err(|error| error.to_string())
        }
        Err(minisign_verify::Error::UnsupportedLegacyMode) => {
            if std::fs::metadata(path)
                .map_err(|error| error.to_string())?
                .len()
                > 128 * 1024 * 1024
            {
                return Err("Legacy signature artifact exceeds verification memory limit".into());
            }
            key.verify(
                &std::fs::read(path).map_err(|error| error.to_string())?,
                &signature,
                true,
            )
            .map_err(|error| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// Hold the verified installer against modification/replacement until launch.
#[cfg(target_os = "windows")]
pub fn launch_verified_installer(
    path: &Path,
    signature: &str,
    launch: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    with_verified_installer(path, signature, PUBLIC_KEY, launch)
}

#[cfg(target_os = "windows")]
fn with_verified_installer(
    path: &Path,
    signature: &str,
    public_key: &str,
    launch: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    use std::os::windows::fs::OpenOptionsExt;
    let path = path.canonicalize().map_err(|error| error.to_string())?;
    if path.extension().and_then(|extension| extension.to_str()) != Some("exe") {
        return Err("Update installer must be an executable".into());
    }
    // FILE_SHARE_READ permits the loader, but excludes write and delete sharing.
    let guard = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .map_err(|error| error.to_string())?;
    verify_file(&path, signature, public_key)?;
    let result = launch(&path);
    drop(guard);
    result
}

pub async fn download(
    release: &Release,
    directory: &Path,
    cancelled: &CancellationToken,
    progress: impl Fn(DownloadProgress),
) -> Result<PathBuf, String> {
    let version = semver::Version::parse(&release.version).map_err(|error| error.to_string())?;
    let extension = match release.target.as_str() {
        "windows-x86_64" | "windows-x86_64-nsis" => "exe",
        "darwin-aarch64" | "darwin-aarch64-app" => "app.tar.gz",
        _ => return Err("Unsupported update platform".into()),
    };
    if cancelled.is_cancelled() {
        return Err("Update download cancelled".into());
    }
    decode_signature(&release.artifact.signature, PUBLIC_KEY)?;
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|error| error.to_string())?;
    let url = url::Url::parse(&release.artifact.url).map_err(|error| error.to_string())?;
    if url.scheme() != "https" {
        return Err("Update artifacts require HTTPS".into());
    }
    tokio::fs::create_dir_all(directory)
        .await
        .map_err(|error| error.to_string())?;
    let temporary = tempfile::Builder::new()
        .prefix("download-")
        .tempfile_in(directory)
        .map_err(|error| error.to_string())?;
    let temporary_path = temporary.path().to_path_buf();
    let mut file =
        tokio::fs::File::from_std(temporary.reopen().map_err(|error| error.to_string())?);
    let response = tokio::select! { _ = cancelled.cancelled() => return Err("Update download cancelled".into()), result = client
        .get(url)
        .send()
        => result.map_err(|error| error.without_url().to_string())? }
        .error_for_status()
        .map_err(|error| error.without_url().to_string())?;
    let total = response.content_length();
    if total.is_some_and(|length| length > MAX_DOWNLOAD) {
        return Err("Update is too large".into());
    }
    let mut downloaded = 0;
    let mut last_progress = std::time::Instant::now();
    let mut stream = response.bytes_stream();
    loop {
        let chunk = tokio::select! { _ = cancelled.cancelled() => return Err("Update download cancelled".into()), chunk = stream.next() => chunk };
        let Some(chunk) = chunk else {
            break;
        };
        let chunk = chunk.map_err(|error| error.to_string())?;
        downloaded += chunk.len() as u64;
        if downloaded > MAX_DOWNLOAD {
            return Err("Update is too large".into());
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
        if last_progress.elapsed() >= Duration::from_millis(100) {
            progress(DownloadProgress { downloaded, total });
            last_progress = std::time::Instant::now();
        }
    }
    progress(DownloadProgress { downloaded, total });
    file.sync_all().await.map_err(|error| error.to_string())?;
    drop(file);
    if cancelled.is_cancelled() {
        return Err("Update download cancelled".into());
    }
    let checked = temporary_path.clone();
    let signature = release.artifact.signature.clone();
    tokio::task::spawn_blocking(move || verify_file(&checked, &signature, PUBLIC_KEY))
        .await
        .map_err(|error| error.to_string())??;
    if cancelled.is_cancelled() {
        return Err("Update download cancelled".into());
    }
    let destination = directory.join(format!("Rotor-{version}.{extension}"));
    temporary
        .persist(&destination)
        .map_err(|error| error.to_string())?;
    Ok(destination)
}

pub fn target() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x86_64"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "darwin-aarch64"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_metadata_shape_is_understood_without_downgrading() {
        let manifest: Manifest = serde_json::from_str(include_str!(
            "../../../doc/gpui-migration/evidence/v2.6.0-latest.json"
        ))
        .unwrap();
        assert!(select_release(manifest.clone(), "2.6.0", "windows-x86_64")
            .unwrap()
            .is_none());
        assert!(select_release(manifest, "2.5.0", "darwin-aarch64")
            .unwrap()
            .unwrap()
            .artifact
            .url
            .ends_with(".app.tar.gz"));
    }
    #[test]
    fn missing_platform_insecure_urls_and_bad_signatures_fail_closed() {
        let manifest = Manifest {
            version: "2.7.0".into(),
            notes: String::new(),
            pub_date: None,
            platforms: HashMap::from([(
                "windows-x86_64".into(),
                Artifact {
                    signature: "bad".into(),
                    url: "http://example.com/update.exe".into(),
                },
            )]),
        };
        assert!(select_release(manifest.clone(), "2.6.0", "darwin-aarch64").is_err());
        assert!(select_release(manifest, "2.6.0", "windows-x86_64").is_err());
        assert!(verify(b"tampered", "bad", PUBLIC_KEY).is_err());
    }
    // Public test vectors from minisign-verify 0.2.5 (MIT), not production keys.
    #[test]
    fn signed_stream_and_legacy_vectors_reject_tampering() {
        let key = BASE64_STANDARD.encode(
            "untrusted comment: test key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3",
        );
        for signature in [
            "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==",
            "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==",
        ] {
            let signature = BASE64_STANDARD.encode(signature);
            let file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(file.path(), b"test").unwrap();
            verify(b"test", &signature, &key).unwrap();
            verify_file(file.path(), &signature, &key).unwrap();
            #[cfg(target_os = "windows")]
            {
                let directory = tempfile::tempdir().unwrap();
                let installer = directory.path().join("fixture.exe");
                std::fs::write(&installer, b"test").unwrap();
                with_verified_installer(&installer, &signature, &key, |path| {
                    assert!(std::fs::write(path, b"tampered").is_err());
                    assert!(std::fs::rename(path, directory.path().join("replacement.exe")).is_err());
                    Ok(())
                }).unwrap();
                std::fs::write(&installer, b"tampered").unwrap();
                assert!(with_verified_installer(&installer, &signature, &key, |_| panic!("must not launch unverified bytes")).is_err());
            }

            std::fs::write(file.path(), b"Test").unwrap();
            assert!(verify_file(file.path(), &signature, &key).is_err());
            assert!(verify(b"test", &signature, PUBLIC_KEY).is_err());
        }
    }

    #[tokio::test]
    async fn invalid_downloads_do_not_create_staging_directories() {
        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("updates");
        let mut release = Release {
            version: "../escape".into(),
            notes: String::new(),
            target: "windows-x86_64".into(),
            artifact: Artifact {
                signature: "bad".into(),
                url: "https://example.com/update.exe".into(),
            },
        };
        assert!(
            download(&release, &staging, &CancellationToken::new(), |_| {})
                .await
                .is_err()
        );
        release.version = "2.7.0".into();
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(download(&release, &staging, &cancelled, |_| {})
            .await
            .is_err());
        assert!(!staging.exists());
        assert!(check(&[], "2.6.0", "windows-x86_64").await.is_err());
    }

    #[test]
    fn native_and_legacy_public_keys_are_identical() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../../src-tauri/tauri.conf.json")).unwrap();
        assert_eq!(
            PUBLIC_KEY.trim(),
            config["plugins"]["updater"]["pubkey"].as_str().unwrap()
        );
    }
}
