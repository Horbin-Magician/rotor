use crate::idx_store::IDX_STORE;
use crate::kv_store::CONF_STORE;
use crate::{utils, walk_exec, watch_exec};
use log::info;

#[cfg(windows)]
use log::error;
#[cfg(windows)]
use std::sync::mpsc;
#[cfg(windows)]
use std::sync::mpsc::Sender;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use crate::usn_journal_watcher::Watcher;
#[cfg(windows)]
const STORE_PATH: &'static str = "orangecachedata";
#[cfg(windows)]
const RECYCLE_PATH: &'static str = "$RECYCLE.BIN";

fn do_run() {
  // reindex
  walk_exec::run();

  IDX_STORE.disable_full_indexing();
  
  // watch
  win_watch();       // for windows
  watch_exec::run(); // for unix
}

#[cfg(windows)]
fn win_watch() -> bool {
  let (tx, rx) = mpsc::channel();
  let nos = utils::get_win32_ready_drive_nos();

  for no in nos {
    let volume_path = utils::build_volume_path(no.as_str());
    let tx_clone = tx.clone();
    start_usn_watch(no, volume_path, tx_clone);
  }

  let success = rx.recv().unwrap();
  success
}

#[cfg(windows)]
unsafe fn start_usn_watch<'a>(no: String, volume_path: String, tx_clone: Sender<bool>) {
  info!("start_usn_watch {}", volume_path);

  std::thread::spawn(move || {
    let key = format!("usn#next_usn#{}", volume_path.clone());
    let next_usn = CONF_STORE
      .get_str(key.clone())
      .unwrap_or("0".to_string())
      .parse()
      .unwrap();

    let result = Watcher::new(volume_path.as_str(), None, Some(next_usn));
    if result.is_err() {
      error!(" {:?} ", result.err());
      let _ = tx_clone.send(false);
      return;
    }

    let mut watcher = result.unwrap();
    let _ = tx_clone.send(true);
    let mut loaded = false;
    loop {
      let read_res = watcher.read();
      if read_res.is_err() {
        watcher = Watcher::new(volume_path.as_str(), None, Some(0)).unwrap();
        continue;
      }
      let records = read_res.unwrap();
      if records.is_empty() {
        if !loaded {
          loaded = true;
          info!("volume {} usn history loaded", volume_path);
        }
        std::thread::sleep(Duration::from_millis(500));
      } else {
        let usn_no = records.last().unwrap().usn.to_string();

        for record in records {
          let path = record.path.to_str().unwrap();
          let file_name = record.file_name;
          let abs_path = format!("{}:{}", no.as_str(), path);

          if abs_path.contains(STORE_PATH) || abs_path.contains(RECYCLE_PATH) {
            continue;
          }

          let is_dir = std::fs::metadata(abs_path.clone())
            .map(|x| x.is_dir())
            .unwrap_or(false);
          let name0 = file_name.clone();
          let ext = utils::file_ext(&name0);

          IDX_STORE.add(file_name, abs_path.clone(), is_dir, ext.to_string());
        }

        CONF_STORE.put_str(key.clone(), usn_no);
      }
    }
  });
}
