use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Every index fixture has its own file; no global profile or drive is opened.
pub(super) struct IndexFile(PathBuf);

impl IndexFile {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rotor-index-release-{}-{}.fd",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("unique synthetic index");
        Self(path)
    }

    pub(super) fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for IndexFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "windows")]
pub(super) fn private_commit_bytes() -> usize {
    use windows::Win32::System::{
        ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
        Threading::GetCurrentProcess,
    };
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            std::ptr::from_mut(&mut counters).cast(),
            std::mem::size_of_val(&counters) as u32,
        )
        .expect("private commit query");
    }
    counters.PrivateUsage
}
