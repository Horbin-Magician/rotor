//! A profile-scoped file lock plus a loopback activation endpoint. The endpoint
//! accepts only activation; it cannot execute commands or read application data.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

const IO_TIMEOUT: Duration = Duration::from_millis(300);
const MAX_METADATA: u64 = 2048;

#[derive(Serialize, Deserialize)]
struct Endpoint {
    address: SocketAddr,
    token: String,
}

pub enum Instance {
    Primary(InstanceGuard),
    ActivatedExisting,
}

pub struct InstanceGuard {
    _lock: File,
    address: SocketAddr,
    metadata: PathBuf,
    stopping: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl InstanceGuard {
    pub fn acquire(directory: &Path, activate: impl Fn() + Send + 'static) -> io::Result<Instance> {
        Self::acquire_mode(directory, activate, None)
    }

    pub fn acquire_after_exit(
        directory: &Path,
        timeout: Duration,
        activate: impl Fn() + Send + 'static,
    ) -> io::Result<Instance> {
        Self::acquire_mode(directory, activate, Some(timeout))
    }

    fn acquire_mode(
        directory: &Path,
        activate: impl Fn() + Send + 'static,
        wait: Option<Duration>,
    ) -> io::Result<Instance> {
        fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        let metadata = directory.join(".native-instance.json");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join(".native-instance.lock"))?;
        let attempts = wait
            .map(|duration| (duration.as_millis() / 50).max(1) as usize)
            .unwrap_or(8);
        for _ in 0..attempts {
            match file.try_lock() {
                Ok(()) => return Self::start(file, metadata, activate).map(Instance::Primary),
                Err(TryLockError::Error(error)) => return Err(error),
                Err(TryLockError::WouldBlock) => {
                    if wait.is_none() && forward(&metadata).is_ok() {
                        return Ok(Instance::ActivatedExisting);
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "existing instance could not be activated",
        ))
    }

    fn start(
        file: File,
        metadata: PathBuf,
        activate: impl Fn() + Send + 'static,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|error| io::Error::other(error.to_string()))?;
        let token = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let expected = format!("ROTOR-ACTIVATE {token}");
        let stopping = Arc::new(AtomicBool::new(false));
        let stop = stopping.clone();
        let worker = thread::Builder::new()
            .name("rotor-instance".into())
            .spawn(move || {
                for connection in listener.incoming() {
                    if stop.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(mut stream) = connection else {
                        break;
                    };
                    if stream.set_read_timeout(Some(IO_TIMEOUT)).is_err()
                        || stream.set_write_timeout(Some(IO_TIMEOUT)).is_err()
                    {
                        continue;
                    }
                    let mut request = String::new();
                    let read = BufReader::new((&mut stream).take(128)).read_line(&mut request);
                    if read.is_ok() && request.trim() == expected && !stop.load(Ordering::Acquire) {
                        activate();
                        let _ = stream.write_all(b"OK\n");
                    } else {
                        let _ = stream.write_all(b"DENIED\n");
                    }
                }
            })?;
        let guard = Self {
            _lock: file,
            address,
            metadata,
            stopping,
            worker: Some(worker),
        };
        let bytes = serde_json::to_vec(&Endpoint { address, token }).map_err(io::Error::other)?;
        rotor_common::persistence::atomic_write_private(&guard.metadata, &bytes)?;
        Ok(guard)
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.address, IO_TIMEOUT);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // The lock is still held while the endpoint is removed. Never unlink
        // the lock file itself: replacing its inode would permit two owners.
        let _ = fs::remove_file(&self.metadata);
    }
}

fn forward(metadata: &Path) -> io::Result<()> {
    let mut bytes = Vec::new();
    File::open(metadata)?
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "instance metadata is too large",
        ));
    }
    let endpoint: Endpoint = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    if !endpoint.address.ip().is_loopback()
        || endpoint.token.len() != 32
        || !endpoint.token.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid local activation endpoint",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&endpoint.address, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    writeln!(stream, "ROTOR-ACTIVATE {}", endpoint.token)?;
    let mut reply = String::new();
    BufReader::new(stream.take(16)).read_line(&mut reply)?;
    if reply.trim() != "OK" {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "activation was rejected",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_waits_for_lock_release_without_activating_the_old_process() {
        let directory = tempfile::tempdir().unwrap();
        let (notice, received) = std::sync::mpsc::channel();
        let Instance::Primary(primary) = InstanceGuard::acquire(directory.path(), move || {
            let _ = notice.send(());
        })
        .unwrap() else {
            panic!("expected primary");
        };
        let path = directory.path().to_path_buf();
        let waiting = std::thread::spawn(move || {
            InstanceGuard::acquire_after_exit(&path, Duration::from_secs(2), || {})
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(received.try_recv().is_err());
        drop(primary);
        assert!(matches!(
            waiting.join().unwrap().unwrap(),
            Instance::Primary(_)
        ));
    }
    use std::sync::mpsc;

    #[test]
    fn another_launch_activates_primary_and_lock_is_released_on_drop() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let Instance::Primary(primary) = InstanceGuard::acquire(directory.path(), move || {
            let _ = sender.send(());
        })
        .unwrap() else {
            panic!("expected primary");
        };
        assert!(matches!(
            InstanceGuard::acquire(directory.path(), || {}).unwrap(),
            Instance::ActivatedExisting
        ));
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(primary);
        assert!(matches!(
            InstanceGuard::acquire(directory.path(), || {}).unwrap(),
            Instance::Primary(_)
        ));
    }

    #[test]
    fn invalid_tokens_do_not_activate_and_idle_clients_do_not_block_exit() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let Instance::Primary(primary) = InstanceGuard::acquire(directory.path(), move || {
            let _ = sender.send(());
        })
        .unwrap() else {
            panic!("expected primary");
        };
        let mut client = TcpStream::connect(primary.address).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        client.write_all(b"ROTOR-ACTIVATE invalid\n").unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        assert_eq!(reply.trim(), "DENIED");
        assert!(receiver.try_recv().is_err());
        let _idle = TcpStream::connect(primary.address).unwrap();
        let start = std::time::Instant::now();
        drop(primary);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
