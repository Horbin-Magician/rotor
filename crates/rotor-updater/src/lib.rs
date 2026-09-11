pub mod bundle;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{handoff_error, launch_handoff, run_helper};

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
    "https://gitee.com/horbin/rotor/releases/download/native-preview/native-preview.json",
    "https://github.com/Horbin-Magician/rotor/releases/download/native-preview/native-preview.json",
];
pub const STABLE_ENDPOINTS: &[&str] = &[
    "https://gitee.com/horbin/rotor/releases/download/native-stable/native-stable.json",
    "https://github.com/Horbin-Magician/rotor/releases/download/native-stable/native-stable.json",
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
    pub schema_version: u32,
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
    if manifest.schema_version != 1 {
        return Err("Unsupported native update manifest".into());
    }
    let version = semver::Version::parse(&manifest.version).map_err(|error| error.to_string())?;
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
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err("Update is too large".into());
    }
    let (key, signature) = decode_signature(signature, public_key)?;
    key.verify(bytes, &signature, false)
        .map_err(|error| error.to_string())
}

fn decode_signature(signature: &str, public_key: &str) -> Result<(PublicKey, Signature), String> {
    let key = PublicKey::decode(public_key.trim()).map_err(|error| error.to_string())?;
    let signature = Signature::decode(signature.trim()).map_err(|error| error.to_string())?;

    Ok((key, signature))
}

pub fn verify_file(path: &Path, signature: &str, public_key: &str) -> Result<(), String> {
    use std::io::Read;
    let (key, signature) = decode_signature(signature, public_key)?;
    if std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_DOWNLOAD
    {
        return Err("Update is too large".into());
    }
    let mut verifier = key
        .verify_stream(&signature)
        .map_err(|error| error.to_string())?;
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut read = 0_u64;
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > MAX_DOWNLOAD {
            return Err("Update is too large".into());
        }
        verifier.update(&buffer[..count]);
    }
    verifier.finalize().map_err(|error| error.to_string())
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
    download_with_client(release, directory, cancelled, progress, &client, PUBLIC_KEY).await
}

// Transport/key injection is private so production callers cannot bypass HTTPS
// or substitute a test signing key. Tests exercise the actual streaming path.
async fn download_with_client(
    release: &Release,
    directory: &Path,
    cancelled: &CancellationToken,
    progress: impl Fn(DownloadProgress),
    client: &reqwest::Client,
    public_key: &str,
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
    decode_signature(&release.artifact.signature, public_key)?;
    let url = url::Url::parse(&release.artifact.url).map_err(|error| error.to_string())?;
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
    let public_key = public_key.to_owned();
    tokio::task::spawn_blocking(move || verify_file(&checked, &signature, &public_key))
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
    fn missing_platform_insecure_urls_and_bad_signatures_fail_closed() {
        let manifest = Manifest {
            schema_version: 1,
            version: "3.1.0".into(),
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
        assert!(select_release(manifest.clone(), "3.0.0", "darwin-aarch64").is_err());
        assert!(select_release(manifest, "3.0.0", "windows-x86_64").is_err());
        assert!(verify(b"tampered", "bad", PUBLIC_KEY).is_err());
    }
    // Public test vectors from minisign-verify 0.2.5 (MIT), not production keys.
    #[test]
    fn signed_stream_rejects_tampering() {
        let key =
            "untrusted comment: test key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3"
                .to_string();
        for signature in [
            "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==",
        ] {
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
            assert!(verify(b"tes", &signature, &key).is_err());
            assert!(verify(b"test", &signature[..signature.len() / 2], &key).is_err());
            let unsupported = signature.replacen("\nRUQ", "\nRWQ", 1);
            assert!(verify(b"test", &unsupported, &key).is_err());
            assert!(verify_file(file.path(), &unsupported, &key).is_err());
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
        release.version = "3.1.0".into();
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert!(download(&release, &staging, &cancelled, |_| {})
            .await
            .is_err());
        assert!(!staging.exists());
        assert!(check(&[], "3.0.0", "windows-x86_64").await.is_err());
    }

    #[tokio::test]
    async fn transport_failures_preserve_previous_download_and_retry_cleanly() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        use std::io::{Read, Write};
        let key =
            "untrusted comment: test key\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3"
                .to_string();
        let signature = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1556193335\tfile:test\ny/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==".to_string();
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("Rotor-3.1.0.exe");
        std::fs::write(&destination, b"previous verified artifact").unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        // Real socket failures, including a truncated body and a validly encoded
        // signature over different bytes, must leave no partial download behind.
        for (response, cancel, success) in [
            (
                "HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n",
                false,
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\ntest",
                false,
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nTest",
                false,
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 1073741825\r\n\r\n",
                false,
                false,
            ),
            ("", true, false),
            (
                "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ntest",
                false,
                true,
            ),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/update.exe", listener.local_addr().unwrap());
            let cancelled = CancellationToken::new();
            let server_cancel = cancelled.clone();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = [0_u8; 4096];
                assert!(socket.read(&mut request).unwrap() > 0);
                if cancel {
                    server_cancel.cancel();
                } else {
                    socket.write_all(response.as_bytes()).unwrap();
                }
            });
            let release = Release {
                version: "3.1.0".into(),
                notes: String::new(),
                target: "windows-x86_64".into(),
                artifact: Artifact {
                    signature: signature.clone(),
                    url,
                },
            };
            let result = download_with_client(
                &release,
                directory.path(),
                &cancelled,
                |_| {},
                &client,
                &key,
            )
            .await;
            server.join().unwrap();
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(
                std::fs::read(&destination).unwrap(),
                if success {
                    b"test".as_slice()
                } else {
                    b"previous verified artifact".as_slice()
                }
            );
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        }
    }
}
