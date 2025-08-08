use crate::fs_watcher::FsWatcher;

#[cfg(windows)]
use crate::utils::get_win32_ready_drives;

pub fn run() {
  #[cfg(windows)]
  win_run();

  #[cfg(target_os = "macos")]
  macos_run();
}

#[cfg(target_os = "macos")]
fn macos_run() {
  let mut watcher = FsWatcher::new("/".to_string());
  std::thread::spawn(move || {
    watcher.start();
  });
}

#[cfg(windows)]
fn win_run() {
  let drives = unsafe { get_win32_ready_drives() };
  for driv in drives {
    std::thread::spawn(move || {
      let mut watcher = FsWatcher::new(driv);
      watcher.start();
    });
  }
}
