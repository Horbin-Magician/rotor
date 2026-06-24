use std::cmp::Ordering as CmpOrdering;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};
use std::sync::atomic::{AtomicBool, Ordering};

use super::search_match::{make_filter, match_indexed_name, prepare_search_name, SearchAlias};
use super::{read_string, read_u16, read_u32, read_u8, SearchResultItem};
use rotor_platform::file_util;

type DirId = u32;

const ROOT_DIR_ID: DirId = 0;
const INDEX_MAGIC: [u8; 4] = *b"RDFM";
const INDEX_VERSION: u8 = 2;

#[derive(Clone)]
struct DirNode {
    parent_id: DirId,
    name: String,
}

#[derive(Hash, PartialEq, Eq)]
struct DirLookupKey {
    parent_id: DirId,
    name: String,
}

struct DirectoryTree {
    nodes: Vec<DirNode>,
    lookup: HashMap<DirLookupKey, DirId>,
}

impl DirectoryTree {
    fn new() -> Self {
        Self {
            nodes: vec![DirNode {
                parent_id: ROOT_DIR_ID,
                name: String::new(),
            }],
            lookup: HashMap::new(),
        }
    }

    fn clear(&mut self) {
        self.nodes.truncate(1);
        self.lookup.clear();
    }

    fn contains(&self, dir_id: DirId) -> bool {
        (dir_id as usize) < self.nodes.len()
    }

    fn intern_path(&mut self, path: &Path) -> DirId {
        let mut dir_id = ROOT_DIR_ID;
        for name in path_component_names(path) {
            dir_id = self.intern_child(dir_id, name);
        }
        dir_id
    }

    fn find_path(&self, path: &Path) -> Option<DirId> {
        let mut dir_id = ROOT_DIR_ID;
        for name in path_component_names(path) {
            dir_id = self.find_child(dir_id, &name)?;
        }
        Some(dir_id)
    }

    fn insert_loaded(&mut self, parent_id: DirId, name: String) -> io::Result<DirId> {
        if !self.contains(parent_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Directory parent id is out of range",
            ));
        }

        let dir_id = self.next_id()?;
        let key = DirLookupKey {
            parent_id,
            name: name.clone(),
        };
        if self.lookup.insert(key, dir_id).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Duplicate directory node in index",
            ));
        }

        self.nodes.push(DirNode { parent_id, name });
        Ok(dir_id)
    }

    fn path(&self, dir_id: DirId) -> Option<String> {
        if !self.contains(dir_id) {
            return None;
        }

        if dir_id == ROOT_DIR_ID {
            return Some(String::new());
        }

        let mut segments = Vec::new();
        let mut current_id = dir_id;
        while current_id != ROOT_DIR_ID {
            let node = self.nodes.get(current_id as usize)?;
            segments.push(node.name.as_str());
            current_id = node.parent_id;

            if segments.len() >= self.nodes.len() {
                return None;
            }
        }

        let mut path = PathBuf::new();
        for segment in segments.iter().rev() {
            if path.as_os_str().is_empty() {
                path = PathBuf::from(segment);
            } else {
                path.push(segment);
            }
        }

        Some(path.to_string_lossy().into_owned())
    }

    fn intern_child(&mut self, parent_id: DirId, name: String) -> DirId {
        let key = DirLookupKey {
            parent_id,
            name: name.clone(),
        };
        if let Some(dir_id) = self.lookup.get(&key) {
            return *dir_id;
        }

        let dir_id = self
            .next_id()
            .unwrap_or_else(|_| panic!("Directory tree exceeded u32::MAX nodes"));
        self.nodes.push(DirNode { parent_id, name });
        self.lookup.insert(key, dir_id);
        dir_id
    }

    fn find_child(&self, parent_id: DirId, name: &str) -> Option<DirId> {
        self.lookup
            .get(&DirLookupKey {
                parent_id,
                name: name.to_string(),
            })
            .copied()
    }

    fn next_id(&self) -> io::Result<DirId> {
        DirId::try_from(self.nodes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Directory tree exceeded u32::MAX nodes",
            )
        })
    }
}

fn path_component_names(path: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending_prefix = None;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                pending_prefix = Some(prefix.as_os_str().to_string_lossy().into_owned());
            }
            Component::RootDir => {
                let name = match pending_prefix.take() {
                    Some(mut prefix) => {
                        prefix.push(MAIN_SEPARATOR);
                        prefix
                    }
                    None => MAIN_SEPARATOR.to_string(),
                };
                names.push(name);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(prefix) = pending_prefix.take() {
                    names.push(prefix);
                }
                names.push("..".to_string());
            }
            Component::Normal(segment) => {
                if let Some(prefix) = pending_prefix.take() {
                    names.push(prefix);
                }
                names.push(segment.to_string_lossy().into_owned());
            }
        }
    }

    if let Some(prefix) = pending_prefix {
        names.push(prefix);
    }

    names
}

pub struct FileView {
    pub parent_id: DirId,
    pub file_name: String,
    pub filter: u32,
    pub rank: i8,
    pub aliases: Option<Box<[String]>>,
    pub search_aliases: Option<Box<[SearchAlias]>>,
}

impl PartialEq for FileView {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank
            && self.parent_id == other.parent_id
            && self.file_name == other.file_name
    }
}

impl Eq for FileView {}

impl PartialOrd for FileView {
    fn partial_cmp(&self, other: &Self) -> Option<CmpOrdering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileView {
    fn cmp(&self, other: &Self) -> CmpOrdering {
        self.rank
            .cmp(&other.rank)
            .then_with(|| self.parent_id.cmp(&other.parent_id))
            .then_with(|| self.file_name.cmp(&other.file_name))
    }
}

pub struct FileMap {
    main_set: BTreeSet<FileView>,
    dir_tree: DirectoryTree,
}

impl FileMap {
    pub fn new() -> FileMap {
        FileMap {
            main_set: BTreeSet::new(),
            dir_tree: DirectoryTree::new(),
        }
    }

    pub fn insert(&mut self, file_name: String, path: String) {
        let parent_id = self.dir_tree.intern_path(Path::new(&path));
        let aliases = self.app_aliases(&file_name, &path);
        self.insert_with_aliases(file_name, parent_id, aliases);
    }

    fn insert_with_aliases(
        &mut self,
        file_name: String,
        parent_id: DirId,
        aliases: Vec<String>,
    ) {
        let rank = Self::get_file_rank(&file_name);
        let prepared = prepare_search_name(
            &file_name,
            (!aliases.is_empty()).then_some(aliases.as_slice()),
        );

        self.insert_simple(FileView {
            file_name,
            parent_id,
            filter: prepared.filter,
            rank,
            aliases: (!aliases.is_empty()).then(|| aliases.into_boxed_slice()),
            search_aliases: prepared.aliases,
        });
    }

    fn app_aliases(&self, file_name: &str, path: &str) -> Vec<String> {
        if !file_name.ends_with(".app") {
            return Vec::new();
        }

        let app_path = Path::new(path).join(file_name);
        let Ok(trans_names) = file_util::get_app_trans_names(&app_path) else {
            return Vec::new();
        };

        trans_names
            .values()
            .filter(|name| !file_name.contains(name.as_str()))
            .cloned()
            .collect()
    }

    fn insert_simple(&mut self, file: FileView) {
        self.main_set.replace(file);
    }

    pub fn remove(&mut self, file_name: String, path: String) {
        let Some(parent_id) = self.dir_tree.find_path(Path::new(&path)) else {
            return;
        };
        let rank = Self::get_file_rank(&file_name);
        let file = FileView {
            file_name,
            parent_id,
            filter: 0,
            rank,
            aliases: None,
            search_aliases: None,
        };
        self.main_set.remove(&file);
    }

    pub fn search(
        &self,
        query: &str,
        last_search_num: usize,
        batch: u8,
        cancel: &AtomicBool,
    ) -> (Option<Vec<SearchResultItem>>, usize) {
        let mut result = Vec::new();
        let mut find_num = 0;
        let mut search_num: usize = 0;
        let query_lower = query.to_lowercase();
        let query_filter = make_filter(&query_lower);

        let file_map_iter = self.iter().rev().skip(last_search_num);
        for file in file_map_iter {
            if cancel.load(Ordering::Relaxed) {
                return (None, 0);
            }
            search_num += 1;

            if let Some(file_alias) = match_indexed_name(
                &file.file_name,
                file.aliases.as_deref(),
                file.search_aliases.as_deref(),
                file.filter,
                &query_lower,
                query_filter,
            ) {
                let Some((path, file_path)) = self.result_paths(file.parent_id, &file.file_name)
                else {
                    continue;
                };

                result.push(SearchResultItem {
                    path,
                    file_path,
                    file_name: file.file_name.clone(),
                    rank: file.rank,
                    icon_data: None,
                    alias: file_alias,
                });

                find_num += 1;
                if find_num >= batch {
                    break;
                }
            }
        }

        (Some(result), search_num)
    }

    fn result_paths(&self, parent_id: DirId, file_name: &str) -> Option<(String, String)> {
        let path = self.dir_tree.path(parent_id)?;
        let file_path = Path::new(&path)
            .join(file_name)
            .to_string_lossy()
            .into_owned();
        Some((path, file_path))
    }

    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let save_file = fs::File::create(path)?;
        let mut writer = io::BufWriter::new(save_file);

        writer.write_all(&INDEX_MAGIC)?;
        writer.write_all(&INDEX_VERSION.to_be_bytes())?;

        let dir_count = self.dir_tree.nodes.len().saturating_sub(1) as u32;
        writer.write_all(&dir_count.to_be_bytes())?;
        for node in self.dir_tree.nodes.iter().skip(1) {
            writer.write_all(&node.parent_id.to_be_bytes())?;
            writer.write_all(&(node.name.len() as u16).to_be_bytes())?;
            writer.write_all(node.name.as_bytes())?;
        }

        writer.write_all(&(self.main_set.len() as u32).to_be_bytes())?;
        for file in self.iter() {
            writer.write_all(&file.parent_id.to_be_bytes())?;
            writer.write_all(&(file.file_name.len() as u16).to_be_bytes())?;
            writer.write_all(file.file_name.as_bytes())?;

            let aliases = file.aliases.as_deref().unwrap_or(&[]);
            writer.write_all(&(aliases.len() as u16).to_be_bytes())?;
            for alias in aliases {
                writer.write_all(&(alias.len() as u16).to_be_bytes())?;
                writer.write_all(alias.as_bytes())?;
            }
        }
        writer.flush()?;

        Ok(())
    }

    pub fn read(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let save_file = fs::File::open(path)?;
        let mut reader = io::BufReader::new(save_file);

        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != INDEX_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported default file index format",
            )
            .into());
        }

        let version = read_u8(&mut reader)?;
        if version != INDEX_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unsupported default file index version",
            )
            .into());
        }

        let mut next = FileMap::new();
        let dir_count = read_u32(&mut reader)?;
        for _ in 0..dir_count {
            let parent_id = read_u32(&mut reader)?;
            let name_len = read_u16(&mut reader)?;
            let name = read_string(&mut reader, name_len as usize)?;
            next.dir_tree.insert_loaded(parent_id, name)?;
        }

        let file_count = read_u32(&mut reader)?;
        for _ in 0..file_count {
            let parent_id = read_u32(&mut reader)?;
            if !next.dir_tree.contains(parent_id) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "File parent id is out of range",
                )
                .into());
            }

            let file_name_len = read_u16(&mut reader)?;
            let file_name = read_string(&mut reader, file_name_len as usize)?;

            let aliases_count = read_u16(&mut reader)?;
            let mut aliases = Vec::new();
            for _ in 0..aliases_count {
                let alias_len = read_u16(&mut reader)?;
                let alias = read_string(&mut reader, alias_len as usize)?;
                aliases.push(alias);
            }

            next.insert_with_aliases(file_name, parent_id, aliases);
        }

        *self = next;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.main_set.clear();
        self.dir_tree.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.main_set.is_empty()
    }

    pub fn len(&self) -> usize {
        self.main_set.len()
    }

    fn iter(&self) -> std::collections::btree_set::Iter<'_, FileView> {
        self.main_set.iter()
    }

    fn get_file_rank(file_name: &str) -> i8 {
        let mut rank: i8 = 0;

        let file_name_lower = file_name.to_lowercase();
        if file_name_lower.ends_with(".exe") {
            rank += 10;
        } else if file_name_lower.ends_with(".app") || file_name_lower.ends_with(".lnk") {
            rank += 25;
        }

        let tmp = 40i16 - file_name.len() as i16;
        if tmp > 0 {
            rank += tmp as i8;
        }

        rank
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn search_items(file_map: &FileMap, query: &str) -> Vec<SearchResultItem> {
        let cancel = AtomicBool::new(false);
        let (result, _) = file_map.search(query, 0, 10, &cancel);
        result.unwrap_or_default()
    }

    fn search_names(file_map: &FileMap, query: &str) -> Vec<String> {
        search_items(file_map, query)
            .into_iter()
            .map(|item| item.file_name)
            .collect()
    }

    #[test]
    fn search_matches_file_name_by_full_pinyin_and_initials() {
        let mut file_map = FileMap::new();
        file_map.insert("微信.txt".to_string(), "/tmp".to_string());

        assert_eq!(search_names(&file_map, "weixin"), vec!["微信.txt"]);
        assert_eq!(search_names(&file_map, "wx"), vec!["微信.txt"]);
    }

    #[test]
    fn stores_parent_path_once_for_siblings() {
        let mut file_map = FileMap::new();
        let parent = std::env::temp_dir().join("rotor-parent").join("nested");
        let parent_path = parent.to_string_lossy().into_owned();

        file_map.insert("alpha.txt".to_string(), parent_path.clone());
        file_map.insert("beta.txt".to_string(), parent_path.clone());

        let parent_ids = file_map
            .main_set
            .iter()
            .map(|file| file.parent_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(parent_ids.len(), 1);

        let items = search_items(&file_map, "alpha");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, parent_path);
        assert_eq!(
            items[0].file_path,
            parent.join("alpha.txt").to_string_lossy()
        );
    }

    #[test]
    fn save_and_read_round_trips_directory_tree() {
        let mut file_map = FileMap::new();
        let parent = std::env::temp_dir().join("rotor-parent").join("persisted");
        let parent_path = parent.to_string_lossy().into_owned();
        let index_path =
            std::env::temp_dir().join(format!("rotor-default-file-map-{}.fd", std::process::id()));

        file_map.insert("alpha.txt".to_string(), parent_path.clone());
        file_map.insert("beta.txt".to_string(), parent_path.clone());
        file_map
            .save(&index_path.to_string_lossy())
            .expect("save default index");

        let mut restored = FileMap::new();
        restored
            .read(&index_path.to_string_lossy())
            .expect("read default index");

        let items = search_items(&restored, "beta");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, parent_path);
        assert_eq!(items[0].file_path, parent.join("beta.txt").to_string_lossy());

        let _ = fs::remove_file(index_path);
    }

    #[test]
    fn remove_and_clear_update_file_entries() {
        let mut file_map = FileMap::new();
        let parent = std::env::temp_dir().join("rotor-parent").join("mutable");
        let parent_path = parent.to_string_lossy().into_owned();

        assert!(file_map.is_empty());
        file_map.insert("alpha.txt".to_string(), parent_path.clone());
        file_map.insert("beta.txt".to_string(), parent_path.clone());
        assert_eq!(file_map.len(), 2);

        file_map.remove("alpha.txt".to_string(), parent_path);
        assert_eq!(search_names(&file_map, "alpha"), Vec::<String>::new());
        assert_eq!(search_names(&file_map, "beta"), vec!["beta.txt"]);

        file_map.clear();
        assert!(file_map.is_empty());
    }
}
