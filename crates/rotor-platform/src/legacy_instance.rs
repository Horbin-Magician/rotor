//! Hold the old Tauri instance namespace during the production transition.
//! Payloads may only activate settings; they are never interpreted as commands.
use std::io;

fn validate(identifier: &str) -> io::Result<()> {
    if identifier.is_empty()
        || identifier.len() > 64
        || !identifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid legacy instance identifier",
        ));
    }
    Ok(())
}
#[cfg(target_os = "windows")]
pub struct LegacyLease(windows::Win32::Foundation::HANDLE);
#[cfg(target_os = "windows")]
impl LegacyLease {
    pub fn acquire(identifier: &str, _activate: impl Fn() + Send + 'static) -> io::Result<Self> {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS},
                System::Threading::CreateMutexW,
            },
        };
        validate(identifier)?;
        let name: Vec<u16> = format!("{identifier}-sim")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let handle = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) }
            .map_err(io::Error::other)?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = unsafe { CloseHandle(handle) };
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Close the existing Rotor application before starting the native production build",
            ));
        }
        Ok(Self(handle))
    }
}
#[cfg(target_os = "windows")]
impl Drop for LegacyLease {
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(target_os = "macos")]
pub struct LegacyLease {
    path: std::path::PathBuf,
    inode: u64,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(target_os = "macos")]
impl LegacyLease {
    pub fn acquire(identifier: &str, activate: impl Fn() + Send + 'static) -> io::Result<Self> {
        use std::{
            fs,
            io::Read,
            os::unix::{
                fs::{FileTypeExt, MetadataExt, PermissionsExt},
                net::{UnixListener, UnixStream},
            },
            sync::{
                atomic::{AtomicBool, Ordering},
                Arc,
            },
            thread,
            time::Duration,
        };
        validate(identifier)?;
        let path = std::path::PathBuf::from(format!(
            "/tmp/{}_si.sock",
            identifier.replace(['.', '-'], "_")
        ));
        match UnixStream::connect(&path) {
            Ok(_) => return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Close the existing Rotor application before starting the native production build",
            )),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) => {}
            Err(error) => return Err(error),
        }
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::getuid() } {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "Legacy instance path belongs to another owner or file type",
                ));
            }
            fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let inode = fs::symlink_metadata(&path)?.ino();
        let stopped = Arc::new(AtomicBool::new(false));
        let stopping = stopped.clone();
        let worker = thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                let Ok(stream) = stream else {
                    break;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                let _ = std::io::copy(&mut stream.take(65536), &mut std::io::sink());
                if !stopping.load(Ordering::Acquire) {
                    activate();
                }
            }
        });
        Ok(Self {
            path,
            inode,
            stopped,
            worker: Some(worker),
        })
    }
}
#[cfg(target_os = "macos")]
impl Drop for LegacyLease {
    fn drop(&mut self) {
        use std::os::unix::{fs::MetadataExt, net::UnixStream};
        self.stopped
            .store(true, std::sync::atomic::Ordering::Release);
        let _ = UnixStream::connect(&self.path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if std::fs::symlink_metadata(&self.path).is_ok_and(|metadata| metadata.ino() == self.inode)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_identifiers_that_could_escape_the_instance_namespace() {
        for value in ["", "../other", "Global\\other", "bad\0id"] {
            assert!(super::validate(value).is_err());
        }
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn lease_excludes_another_instance_and_releases_its_kernel_object() {
        let identifier = format!(
            "rotor-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let lease = super::LegacyLease::acquire(&identifier, || {}).unwrap();
        assert!(super::LegacyLease::acquire(&identifier, || {}).is_err());
        drop(lease);
        let _again = super::LegacyLease::acquire(&identifier, || {}).unwrap();
    }
}
