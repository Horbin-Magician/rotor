use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::mpsc::{self, SyncSender},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

enum Message {
    Line(String),
    Flush,
    Shutdown(SyncSender<()>),
}
struct FileLog(SyncSender<Message>);
pub struct LogGuard(SyncSender<Message>);
impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= Level::Info && metadata.target().starts_with("rotor")
    }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = self.0.try_send(Message::Line(format!(
            "{stamp} {} [{}] {}",
            record.level(),
            record.target(),
            record.args()
        )));
    }
    fn flush(&self) {
        let _ = self.0.try_send(Message::Flush);
    }
}
impl Drop for LogGuard {
    fn drop(&mut self) {
        let (sender, receiver) = mpsc::sync_channel(1);
        if self.0.try_send(Message::Shutdown(sender)).is_ok() {
            let _ = receiver.recv_timeout(Duration::from_secs(1));
        }
    }
}
fn write_logs(file: File, receiver: mpsc::Receiver<Message>) {
    let mut file = BufWriter::new(file);
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Line(line) => {
                let _ = writeln!(file, "{line}");
                // This runs on the log worker, never on the UI thread. Keep
                // startup/restore errors available even if the process crashes.
                let _ = file.flush();
            }
            Message::Flush => {
                let _ = file.flush();
            }
            Message::Shutdown(reply) => {
                let _ = file.flush();
                let _ = reply.send(());
                return;
            }
        }
    }
}
pub fn initialize(directory: &Path) -> Result<LogGuard, String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = directory.join("rotor.log");
    if fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 1024 * 1024) {
        fs::rename(&path, directory.join("rotor.previous.log"))
            .map_err(|error| error.to_string())?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let (sender, receiver) = mpsc::sync_channel(256);
    std::thread::Builder::new()
        .name("rotor-log".into())
        .spawn(move || write_logs(file, receiver))
        .map_err(|error| error.to_string())?;
    let guard = LogGuard(sender.clone());
    log::set_boxed_logger(Box::new(FileLog(sender))).map_err(|error| error.to_string())?;
    log::set_max_level(LevelFilter::Info);
    Ok(guard)
}
