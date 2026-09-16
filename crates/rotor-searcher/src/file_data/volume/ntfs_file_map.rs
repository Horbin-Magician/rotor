use super::{SearchCursor, SearchPage};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
#[cfg(test)]
use std::fs;
use std::io::{self, Read};
use std::ops::Bound::{Excluded, Unbounded};
use std::sync::atomic::{AtomicBool, Ordering};

use super::super::excluded_dirs::ExcludedDirs;
use super::search_match::{prepare_search_name, SearchAlias, SearchQuery};
use super::{cache, read_i64, read_string, read_u16, read_u64, SearchResultItem};

pub struct FileView {
    pub parent_index: u64,
    pub file_name: String,
    pub filter: u32,
    pub rank: i8,
    pub search_aliases: Option<Box<[SearchAlias]>>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct FileKey {
    rank: i8,
    pub index: u64,
}

pub struct FileMap {
    pub start_usn: i64,
    pub journal_id: u64,
    main_map: BTreeMap<FileKey, FileView>,
    rank_map: HashMap<u64, i8, std::hash::BuildHasherDefault<fxhash::FxHasher>>,
}

impl FileMap {
    pub fn new() -> FileMap {
        FileMap {
            start_usn: 0,
            journal_id: 0,
            main_map: BTreeMap::new(),
            rank_map: HashMap::default(),
        }
    }

    // insert a file to the database by index, file name and parent index
    pub fn insert(&mut self, index: u64, file_name: String, parent_index: u64) {
        let prepared = prepare_search_name(&file_name, None);
        let rank = Self::get_file_rank(&file_name);
        self.insert_simple(
            index,
            FileView {
                parent_index,
                file_name,
                filter: prepared.filter,
                rank,
                search_aliases: prepared.aliases,
            },
        );
    }

    // insert a file to the database by index and file struct
    fn insert_simple(&mut self, index: u64, file: FileView) {
        let key = FileKey {
            rank: file.rank,
            index,
        };
        if let Some(previous_rank) = self.rank_map.insert(index, file.rank) {
            if previous_rank != file.rank {
                self.main_map.remove(&FileKey {
                    rank: previous_rank,
                    index,
                });
            }
        }
        self.main_map.insert(key, file);
    }

    // remove item
    pub fn remove(&mut self, index: &u64) {
        if self.rank_map.contains_key(index) {
            let file_key = FileKey {
                rank: self.rank_map[index],
                index: *index,
            };
            self.main_map.remove(&file_key);
            self.rank_map.remove(index);
        }
    }

    // search for files by query
    pub fn search(
        &self,
        query: &str,
        cursor: Option<&SearchCursor>,
        batch: u8,
        cancel: &AtomicBool,
        excluded_dirs: &ExcludedDirs,
    ) -> Option<SearchPage> {
        let mut result = Vec::new();
        let mut find_num = 0;
        let mut next_cursor = cursor.cloned();
        let mut exhausted = true;
        let mut query = SearchQuery::new(query);

        let bound = cursor.map(|cursor| FileKey {
            rank: cursor.rank,
            index: cursor.id,
        });
        let range = (Unbounded, bound.as_ref().map_or(Unbounded, Excluded));
        for (key, file) in self.main_map.range::<FileKey, _>(range).rev() {
            if cancel.load(Ordering::Relaxed) {
                return None;
            }
            next_cursor = Some(SearchCursor {
                rank: key.rank,
                id: key.index,
                name: String::new(),
            });
            if query
                .match_name(
                    &file.file_name,
                    None,
                    file.search_aliases.as_deref(),
                    file.filter,
                )
                .is_some()
            {
                if let Some(path) = self.get_path(&file.parent_index) {
                    let full_path = format!("{}{}", path, file.file_name);
                    if excluded_dirs.is_excluded_path(std::path::Path::new(&full_path)) {
                        continue;
                    }
                    result.push(SearchResultItem {
                        path,
                        file_path: full_path,
                        file_name: file.file_name.clone(),
                        rank: file.rank,
                        icon: None,
                        alias: None,
                    });
                    find_num += 1;
                    if find_num >= batch {
                        exhausted = false;
                        break;
                    }
                }
            }
        }

        Some(SearchPage {
            items: result,
            cursor: next_cursor,
            exhausted,
        })
    }

    pub fn save(&self, path: &str) -> io::Result<()> {
        cache::write(std::path::Path::new(path), |writer| {
            writer.write_all(b"RNF1")?;
            writer.write_all(&self.journal_id.to_be_bytes())?;
            writer.write_all(&self.start_usn.to_be_bytes())?;
            writer.write_all(&(self.len() as u64).to_be_bytes())?;
            for (key, file) in self.iter() {
                writer.write_all(&key.index.to_be_bytes())?;
                writer.write_all(&file.parent_index.to_be_bytes())?;
                let length = u16::try_from(file.file_name.len())
                    .map_err(|_| cache::invalid("Index name too long"))?;
                writer.write_all(&length.to_be_bytes())?;
                writer.write_all(file.file_name.as_bytes())?;
            }
            Ok(())
        })
    }

    pub fn read(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let mut reader = cache::read(std::path::Path::new(path))?;
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;
        if &magic != b"RNF1" {
            return Err(cache::invalid("Unsupported NTFS index format").into());
        }
        let mut next = Self::new();
        next.journal_id = read_u64(&mut reader)?;
        next.start_usn = read_i64(&mut reader)?;
        let count = read_u64(&mut reader)?;
        for _ in 0..count {
            let index = read_u64(&mut reader)?;
            let parent_index = read_u64(&mut reader)?;
            let file_name_len = read_u16(&mut reader)?;
            let file_name = read_string(&mut reader, file_name_len as usize)?;
            if index == 0 || index == parent_index || next.contains_index(&index) {
                return Err(cache::invalid("Invalid or duplicate file reference").into());
            }
            next.insert(index, file_name, parent_index);
        }
        cache::end(&mut reader)?;
        *self = next;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.main_map.clear();
        // Only whole-index release uses this path. Keep start_usn for reload,
        // but relinquish the hash table allocation instead of retaining it.
        self.rank_map = HashMap::default();
    }

    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.main_map.len()
    }

    pub fn contains_index(&self, index: &u64) -> bool {
        self.rank_map.contains_key(index)
    }

    // get a File by index
    fn get(&self, index: &u64) -> Option<&FileView> {
        if let Some(rank) = self.rank_map.get(index) {
            let file_key = FileKey {
                rank: *rank,
                index: *index,
            };
            return self.main_map.get(&file_key);
        }
        None
    }

    fn iter(&self) -> std::collections::btree_map::Iter<'_, FileKey, FileView> {
        self.main_map.iter()
    }

    // return rank by filename
    fn get_file_rank(file_name: &str) -> i8 {
        let mut rank: i8 = 0;

        if file_name.to_lowercase().ends_with(".exe") {
            rank += 10;
        } else if file_name.to_lowercase().ends_with(".lnk") {
            rank += 25;
        }

        rank += 40usize.saturating_sub(file_name.len()) as i8;

        rank
    }

    // Constructs a path for a directory
    fn get_path(&self, index: &u64) -> Option<String> {
        let mut segments = Vec::new();
        let mut loop_index = *index;
        let mut visited = std::collections::HashSet::new();
        while loop_index != 0 {
            if !visited.insert(loop_index) {
                return None;
            }
            let file = self.get(&loop_index)?;
            segments.push(file.file_name.as_str());
            loop_index = file.parent_index;
        }

        let path_len = segments.iter().map(|segment| segment.len() + 1).sum();
        let mut path = String::with_capacity(path_len);
        for segment in segments.iter().rev() {
            path.push_str(segment);
            path.push('\\');
        }
        Some(path)
    }
}

#[cfg(test)]
mod release_tests {
    use super::*;
    use crate::file_data::volume::release_tests::{private_commit_bytes, IndexFile};

    fn populate(map: &mut FileMap, count: u64) {
        map.insert(1, "X:".into(), 0);
        for index in 0..count {
            let name = if index % 32 == 0 {
                format!("fixture-{index:06}.exe")
            } else {
                format!("other-{index:06}.txt")
            };
            map.insert(index + 2, name, 1);
        }
    }

    fn pages(map: &FileMap, batch: u8) -> Vec<(String, i8)> {
        let mut cursor = None;
        let mut items = Vec::new();
        loop {
            let page = map.search(
                "fixture",
                cursor.as_ref(),
                batch,
                &AtomicBool::new(false),
                &ExcludedDirs::default(),
            );
            let page = page.unwrap();
            items.extend(
                page.items
                    .into_iter()
                    .map(|item| (item.file_path, item.rank)),
            );
            if page.exhausted {
                return items;
            }
            cursor = page.cursor;
        }
    }

    #[test]
    fn corrupt_read_preserves_live_index_and_rename_replaces_old_rank() {
        let index = IndexFile::new();
        let mut map = FileMap::new();
        map.insert(1, "X:".into(), 0);
        map.insert(2, "old.txt".into(), 1);
        map.insert(2, "new-shortcut.lnk".into(), 1);
        assert_eq!(map.len(), 2);
        assert!(map
            .search(
                "old",
                None,
                20,
                &AtomicBool::new(false),
                &ExcludedDirs::default()
            )
            .unwrap()
            .items
            .is_empty());
        map.save(index.path()).unwrap();
        let mut bytes = fs::read(index.path()).unwrap();
        bytes.truncate(bytes.len() - 1);
        fs::write(index.path(), bytes).unwrap();
        assert!(map.read(index.path()).is_err());
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&2).unwrap().file_name, "new-shortcut.lnk");
        map.remove(&2);
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn cyclic_parent_links_terminate_path_resolution() {
        let mut map = FileMap::new();
        map.insert(1, "a".into(), 2);
        map.insert(2, "b".into(), 1);
        assert_eq!(map.get_path(&1), None);
        assert!(map
            .search(
                "*",
                None,
                20,
                &AtomicBool::new(false),
                &ExcludedDirs::default()
            )
            .unwrap()
            .items
            .is_empty());
    }

    #[test]
    fn release_drops_capacity_preserves_usn_and_reloads_all_pages() {
        let mut map = FileMap::new();
        map.start_usn = 123456789;
        populate(&mut map, 4096);
        let expected = pages(&map, 255);
        assert_eq!(expected.len(), 128);
        assert_eq!(pages(&map, 7), expected);
        let index = IndexFile::new();
        map.save(index.path()).unwrap();

        for _ in 0..3 {
            assert!(map.rank_map.capacity() >= 4097);
            map.clear();
            assert_eq!(map.rank_map.capacity(), 0);
            assert!(map.main_map.is_empty());
            assert_eq!(map.start_usn, 123456789);
            assert!(!map.contains_index(&1));
            assert!(pages(&map, 7).is_empty());
            map.read(index.path()).unwrap();
            assert_eq!(pages(&map, 7), expected);
            assert!(map.contains_index(&1));
        }

        for index in 1..=4097 {
            map.remove(&index);
        }
        assert!(map.is_empty());
        assert!(map.rank_map.capacity() > 0);
        map.clear();
        map.clear();
        assert_eq!(map.rank_map.capacity(), 0);
    }

    #[test]
    #[ignore = "manual process commit measurement; run alone with --ignored --exact --nocapture"]
    fn measure_ntfs_index_release_commit() {
        let baseline = private_commit_bytes();
        let mut map = FileMap::new();
        populate(&mut map, 250_000);
        let loaded = private_commit_bytes();
        // Reproduce the previous release, then isolate the newly freed table.
        map.main_map.clear();
        map.rank_map.clear();
        let retained_capacity = map.rank_map.capacity();
        let legacy_release = private_commit_bytes();
        map.clear();
        let released = private_commit_bytes();
        assert_eq!(map.rank_map.capacity(), 0);
        println!("ntfs entries=250001 retained_capacity={retained_capacity} private_commit_bytes baseline={baseline} loaded={loaded} legacy_release={legacy_release} released={released}");
    }
}
