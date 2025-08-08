extern crate notify;

use crate::idx_store::IDX_STORE;
use crate::utils;
use crate::utils::subs;
use log::{error, info};
use notify::{raw_watcher, Op, RawEvent, RecursiveMode, Watcher};
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
#[cfg(target_os = "windows")]
use std::os::windows::fs::MetadataExt;
use std::path;

use std::sync::mpsc::channel;

pub struct FsWatcher {
  path: String,
}

impl FsWatcher {
  pub fn new(path: String) -> FsWatcher {
    FsWatcher { path }
  }

  pub fn start(&mut self) {
    let (tx, rx) = channel();
    let mut watcher = raw_watcher(tx).unwrap();
    let wt_res = watcher.watch(self.path.as_str(), RecursiveMode::Recursive);
    if wt_res.is_err() {
      error!("{:?}", wt_res.err());
      error!("watch {} err ", self.path);
      return;
    }
    info!("fs watcher started");

    loop {
      match rx.recv() {
        Ok(RawEvent {
          path: Some(path),
          op: Ok(op),
          cookie: _,
        }) => {
          let path_str = path.to_str().unwrap();
          let abs_path = path_str.to_string();
          if path_str.contains("orangecachedata") {
            continue;
          }
          if Op::REMOVE & op == Op::REMOVE {
            IDX_STORE._del(abs_path)
          } else {
            let name = utils::path2name(abs_path.clone());
            let name0 = name.clone();
            let ext = utils::file_ext(name0.as_str());

            IDX_STORE.add(name, abs_path, path.is_dir(), ext.to_string())
          }
        }
        Ok(event) => error!("broken event: {:?}", event),
        Err(e) => error!("watch error: {:?}", e),
      }
    }
  }

  fn save_subs(&mut self, parent_str: &str) {
    let subs = subs(parent_str);
    for sub in subs {
      let sub_path = path::Path::new(sub.as_str());
      let name = sub_path
        .file_name()
        .map(|x| x.to_str().unwrap())
        .unwrap_or_default()
        .to_string();

      if let Ok(meta) = sub_path.metadata() {
        let name0 = name.clone();
        let ext = utils::file_ext(&name0);
        IDX_STORE.add(name, sub.clone(), meta.is_dir(), ext.to_string());
      }
    }
  }
}
