use log::{Level, LevelFilter, Log, Metadata, Record};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

enum Message {
    Line(String),
    Flush,
    Shutdown(SyncSender<()>),
}
struct FileLog {
    sender: SyncSender<Message>,
    capture_timing: bool,
    /// Lines rejected by the full channel; reported with the next accepted line.
    dropped: AtomicUsize,
}
pub struct LogGuard(SyncSender<Message>);
impl FileLog {
    fn new(sender: SyncSender<Message>, capture_timing: bool) -> Self {
        Self {
            sender,
            capture_timing,
            dropped: AtomicUsize::new(0),
        }
    }
}
impl Log for FileLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        (metadata.level() <= Level::Info
            || (self.capture_timing && metadata.target() == "rotor_capture_latency"))
            && metadata.target().starts_with("rotor")
    }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let dropped = self.dropped.swap(0, Ordering::Relaxed);
        let mut line = String::new();
        if dropped > 0 {
            line.push_str(&format!(
                "{stamp} WARN [{}] dropped {dropped} log lines\n",
                module_path!()
            ));
        }
        line.push_str(&format!(
            "{stamp} {} [{}] {}",
            record.level(),
            record.target(),
            record.args()
        ));
        if self.sender.try_send(Message::Line(line)).is_err() {
            self.dropped.fetch_add(dropped + 1, Ordering::Relaxed);
        }
    }
    fn flush(&self) {
        let _ = self.sender.try_send(Message::Flush);
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
    let capture_timing = std::env::var_os("ROTOR_CAPTURE_TIMING").is_some_and(|value| value == "1");
    log::set_boxed_logger(Box::new(FileLog::new(sender, capture_timing)))
        .map_err(|error| error.to_string())?;
    log::set_max_level(if capture_timing {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(logger: &FileLog, text: &str) {
        logger.log(
            &Record::builder()
                .level(Level::Info)
                .target("rotor_test")
                .args(format_args!("{text}"))
                .build(),
        );
    }

    #[test]
    fn dropped_lines_are_reported_with_the_next_accepted_line() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let logger = FileLog::new(sender, false);
        record(&logger, "first");
        record(&logger, "second");
        record(&logger, "third");
        let Ok(Message::Line(line)) = receiver.try_recv() else {
            panic!("first line was not queued");
        };
        assert!(line.ends_with(" INFO [rotor_test] first"), "{line}");
        assert!(receiver.try_recv().is_err());
        record(&logger, "fourth");
        let Ok(Message::Line(line)) = receiver.try_recv() else {
            panic!("fourth line was not queued");
        };
        let mut lines = line.lines();
        let notice = lines.next().unwrap();
        assert!(notice.contains(" WARN ["), "{notice}");
        assert!(notice.ends_with("] dropped 2 log lines"), "{notice}");
        assert!(lines.next().unwrap().ends_with(" INFO [rotor_test] fourth"));
        assert_eq!(lines.next(), None);
        record(&logger, "fifth");
        let Ok(Message::Line(line)) = receiver.try_recv() else {
            panic!("fifth line was not queued");
        };
        assert_eq!(line.lines().count(), 1);
    }
}
