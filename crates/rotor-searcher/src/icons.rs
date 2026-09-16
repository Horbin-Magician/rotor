use crate::{latest, QueryId, SearchIconBatch};
use image::RgbaImage;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const MAX_PATHS: usize = 100;
const MAX_CACHE_ENTRIES: usize = 128;
const MAX_CACHE_BYTES: usize = 8 * 1024 * 1024;
const CACHE_TTL: Duration = Duration::from_secs(60);

struct Task {
    id: QueryId,
    paths: Vec<String>,
    cancel: Arc<AtomicBool>,
}

struct CachedIcon {
    path: String,
    pixels: Option<Arc<RgbaImage>>,
    loaded: Instant,
}
#[derive(Default)]
struct Cache {
    entries: VecDeque<CachedIcon>,
    bytes: usize,
}

impl Cache {
    fn get(
        &mut self,
        path: &str,
        load: &mut impl FnMut(&str) -> Option<RgbaImage>,
    ) -> Option<Arc<RgbaImage>> {
        if let Some(index) = self.entries.iter().position(|entry| entry.path == path) {
            let entry = self.entries.remove(index).unwrap();
            if entry.loaded.elapsed() < CACHE_TTL {
                let pixels = entry.pixels.clone();
                self.entries.push_back(entry);
                return pixels;
            }
            self.bytes -= entry
                .pixels
                .as_ref()
                .map_or(0, |pixels| pixels.as_raw().len());
        }
        let pixels = load(path).map(Arc::new);
        let size = pixels.as_ref().map_or(0, |pixels| pixels.as_raw().len());
        if size <= MAX_CACHE_BYTES {
            while self.entries.len() >= MAX_CACHE_ENTRIES || self.bytes + size > MAX_CACHE_BYTES {
                let entry = self.entries.pop_front().unwrap();
                self.bytes -= entry
                    .pixels
                    .as_ref()
                    .map_or(0, |pixels| pixels.as_raw().len());
            }
            self.bytes += size;
            self.entries.push_back(CachedIcon {
                path: path.into(),
                pixels: pixels.clone(),
                loaded: Instant::now(),
            });
        }
        pixels
    }
}

pub(crate) struct IconWorker {
    sender: latest::Sender<Task>,
    cancel: Arc<AtomicBool>,
    id: Option<QueryId>,
    paths: Vec<String>,
}

impl IconWorker {
    pub fn new(callback: impl Fn(SearchIconBatch) + Send + 'static) -> Self {
        Self::with_loader(callback, rotor_platform::file_util::file_icon)
    }

    pub(crate) fn with_loader(
        callback: impl Fn(SearchIconBatch) + Send + 'static,
        mut load: impl FnMut(&str) -> Option<RgbaImage> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = latest::channel::<Task>();
        std::thread::spawn(move || {
            let mut cache = Cache::default();
            while let Ok(task) = receiver.recv() {
                let mut icons = Vec::new();
                for path in task.paths {
                    if task.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let pixels = cache.get(&path, &mut load);
                    if task.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    if let Some(pixels) = pixels {
                        icons.push((path, pixels));
                    }
                    if icons.len() == 8 {
                        callback(SearchIconBatch {
                            id: task.id,
                            icons: std::mem::take(&mut icons),
                        });
                    }
                }
                if !task.cancel.load(Ordering::Acquire) && !icons.is_empty() {
                    callback(SearchIconBatch { id: task.id, icons });
                }
            }
        });
        Self {
            sender,
            cancel: Arc::new(AtomicBool::new(false)),
            id: None,
            paths: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.id = None;
        self.paths = Vec::new();
    }

    pub fn enqueue(&mut self, id: QueryId, append: bool, paths: Vec<String>) {
        if !append {
            self.reset();
        }
        let id = *self.id.get_or_insert(id);
        for path in paths {
            if self.paths.len() == MAX_PATHS {
                break;
            }
            if !self.paths.contains(&path) {
                self.paths.push(path);
            }
        }
        // A replacement pending task includes all visible rows. Fast paging
        // cannot drop an older page's icons; the bounded cache avoids reloading.
        let _ = self.sender.send(Task {
            id,
            paths: self.paths.clone(),
            cancel: self.cancel.clone(),
        });
    }
}

impl Drop for IconWorker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn slow_old_icon_does_not_block_new_query_or_publish_stale_pixels() {
        let (started, start) = mpsc::channel();
        let (resume, wait) = mpsc::channel();
        let (results, receive) = mpsc::channel();
        let mut worker = IconWorker::with_loader(
            move |batch| {
                results.send(batch).unwrap();
            },
            move |path| {
                if path == "old" {
                    started.send(()).unwrap();
                    wait.recv().unwrap();
                }
                Some(RgbaImage::new(1, 1))
            },
        );
        worker.enqueue(QueryId(1), false, vec!["old".into()]);
        start.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.enqueue(QueryId(2), false, vec!["new".into()]);
        resume.send(()).unwrap();
        let batch = receive.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(batch.id, QueryId(2));
        assert_eq!(batch.icons[0].0, "new");
        assert!(receive.try_recv().is_err());
    }

    #[test]
    fn cache_bounds_pixels_and_reuses_positive_and_negative_results() {
        let mut cache = Cache::default();
        let mut calls = 0;
        let mut loader = |_: &str| {
            calls += 1;
            Some(RgbaImage::new(512, 512))
        };
        for index in 0..20 {
            cache.get(&index.to_string(), &mut loader);
        }
        cache.get("19", &mut loader);
        assert_eq!(calls, 20);
        assert!(cache.bytes <= MAX_CACHE_BYTES);
        for index in 0..200 {
            cache.get(&format!("missing{index}"), &mut |_| None);
        }
        assert!(cache.entries.len() <= MAX_CACHE_ENTRIES);
        cache.get("missing199", &mut |_| {
            panic!("negative result should be cached")
        });
    }
}
