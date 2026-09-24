//! Profile-local launch statistics. Writes commit before publication; clearing
//! advances an epoch so an already-running launch cannot restore erased history.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const DAY: u64 = 86_400;
const MAX_RECORDS: usize = 2_000;

#[derive(Clone, Default, Deserialize, Serialize)]
struct Record {
    count: u64,
    last_opened: u64,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Document {
    #[serde(default)]
    records: BTreeMap<String, Record>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Default)]
struct State {
    document: Option<Document>,
}

pub struct UsageStore {
    path: PathBuf,
    state: Mutex<State>,
    epoch: AtomicU64,
}

#[derive(Default)]
pub(crate) struct UsageSnapshot {
    bonuses: HashMap<String, i16>,
    names: HashSet<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// Exact path spelling is retained on macOS (including case-sensitive volumes).
fn key(path: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        path.replace('/', "\\").to_lowercase()
    }
    #[cfg(not(target_os = "windows"))]
    {
        path.to_owned()
    }
}

fn name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn bonus(record: &Record, time: u64) -> i16 {
    if record.count == 0 {
        return 0;
    }
    let days = time.saturating_sub(record.last_opened) / DAY;
    if days >= 90 {
        return 0;
    }
    let recent = match days {
        0 => 24,
        1..=6 => 16,
        7..=29 => 8,
        _ => 0,
    };
    let frequency = (record.count.ilog2() as i16 * 4).min(24);
    recent + if days < 30 { frequency } else { frequency / 2 }
}

impl UsageStore {
    pub fn new(profile: PathBuf) -> Self {
        Self {
            path: profile.join("search-usage.json"),
            state: Mutex::new(State::default()),
            epoch: AtomicU64::new(0),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    fn load(&self, state: &mut State) -> io::Result<()> {
        if state.document.is_none() {
            state.document = Some(match std::fs::read(&self.path) {
                Ok(bytes) => serde_json::from_slice(&bytes)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => Document::default(),
                Err(e) => return Err(e),
            });
        }
        Ok(())
    }

    fn save(&self, document: &Document) -> io::Result<()> {
        rotor_common::persistence::atomic_write_private(&self.path, &serde_json::to_vec(document)?)
    }

    pub fn record_open(&self, path: &str, epoch: u64) -> io::Result<()> {
        self.record_at(path, epoch, now())
    }

    fn record_at(&self, path: &str, epoch: u64, time: u64) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if epoch != self.epoch() {
            return Ok(());
        }
        self.load(&mut state)?;
        let mut next = state.document.clone().unwrap();
        next.records
            .retain(|_, record| time.saturating_sub(record.last_opened) < 90 * DAY);
        let record = next.records.entry(key(path)).or_default();
        record.count = record.count.saturating_add(1);
        record.last_opened = record.last_opened.max(time);
        while next.records.len() > MAX_RECORDS {
            let oldest = next
                .records
                .iter()
                .min_by_key(|(path, r)| (r.last_opened, *path))
                .unwrap()
                .0
                .clone();
            next.records.remove(&oldest);
        }
        self.save(&next)?;
        state.document = Some(next);
        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.load(&mut state)?;
        let mut next = state.document.clone().unwrap();
        next.records.clear();
        self.save(&next)?;
        state.document = Some(next);
        self.epoch.fetch_add(1, Ordering::Release);
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> UsageSnapshot {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(error) = self.load(&mut state) {
            log::warn!("Cannot load search usage: {error}");
            return UsageSnapshot::default();
        }
        let time = now();
        let bonuses: HashMap<_, _> = state
            .document
            .as_ref()
            .unwrap()
            .records
            .iter()
            .filter_map(|(path, record)| {
                let score = bonus(record, time);
                (score > 0).then(|| (path.clone(), score))
            })
            .collect();
        let names = bonuses.keys().map(|path| name(path).to_owned()).collect();
        UsageSnapshot { bonuses, names }
    }
}

impl UsageSnapshot {
    pub fn is_empty(&self) -> bool {
        self.bonuses.is_empty()
    }
    pub fn contains_name(&self, filename: &str) -> bool {
        self.names.contains(&key(filename))
    }
    pub fn bonus(&self, path: &str) -> i16 {
        self.bonuses.get(&key(path)).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_decays_caps_and_preserves_relevance_tiers() {
        let mut r = Record {
            count: 1,
            last_opened: 100 * DAY,
            ..Record::default()
        };
        assert_eq!(bonus(&r, 100 * DAY), 24);
        assert_eq!(bonus(&r, 107 * DAY), 8);
        r.count = u64::MAX;
        assert_eq!(bonus(&r, 100 * DAY), 48);
        assert_eq!(bonus(&r, 130 * DAY), 12);
        assert_eq!(bonus(&r, 190 * DAY), 0);
        assert!(65 + bonus(&r, 100 * DAY) < 128);
    }

    #[test]
    fn failed_clear_keeps_memory_and_epoch_and_corrupt_records_are_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UsageStore::new(dir.path().into());
        store.record_open("kept.txt", 0).unwrap();
        store.path = dir.path().join("blocked");
        std::fs::create_dir(&store.path).unwrap();
        assert!(store.clear().is_err());
        assert_eq!(store.epoch(), 0);
        assert_eq!(store.snapshot().bonus("kept.txt"), 24);
        let corrupt = UsageStore::new(dir.path().join("corrupt"));
        std::fs::create_dir_all(corrupt.path.parent().unwrap()).unwrap();
        std::fs::write(&corrupt.path, b"invalid json").unwrap();
        assert!(corrupt.snapshot().is_empty());
        assert!(corrupt.record_open("new.txt", 0).is_err());
        assert_eq!(std::fs::read(&corrupt.path).unwrap(), b"invalid json");
    }

    #[test]
    fn records_expire_and_storage_has_a_bound() {
        let dir = tempfile::tempdir().unwrap();
        let store = UsageStore::new(dir.path().into());
        let mut doc = Document::default();
        for index in 0..MAX_RECORDS {
            doc.records.insert(
                format!("{index}.txt"),
                Record {
                    count: 1,
                    last_opened: 100 * DAY,
                    ..Record::default()
                },
            );
        }
        store.save(&doc).unwrap();
        store.record_at("new.txt", 0, 101 * DAY).unwrap();
        let state = store.state.lock().unwrap();
        let records = &state.document.as_ref().unwrap().records;
        assert_eq!(records.len(), MAX_RECORDS);
        assert!(records.contains_key("new.txt"));
        drop(state);
        store.record_at("fresh.txt", 0, 200 * DAY).unwrap();
        assert_eq!(
            store
                .state
                .lock()
                .unwrap()
                .document
                .as_ref()
                .unwrap()
                .records
                .len(),
            1
        );
    }

    #[test]
    fn reload_clear_preserves_unknown_fields_and_rejects_old_launches() {
        let dir = tempfile::tempdir().unwrap();
        let store = UsageStore::new(dir.path().into());
        std::fs::write(&store.path, br#"{"future":true,"records":{"test.txt":{"count":4,"last_opened":100,"futureRecord":42}}}"#).unwrap();
        store.record_at("test.txt", 0, 101).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(value["records"]["test.txt"]["count"], 5);
        assert_eq!(value["records"]["test.txt"]["futureRecord"], 42);
        let reloaded = UsageStore::new(dir.path().into());
        reloaded.clear().unwrap();
        reloaded.record_at("late.txt", 0, 102).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(value["future"], true);
        assert_eq!(value["records"], serde_json::json!({}));
        reloaded.record_open("new.txt", reloaded.epoch()).unwrap();
        assert_eq!(reloaded.snapshot().bonus("new.txt"), 24);
    }
}
