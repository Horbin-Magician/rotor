use std::cmp::Ordering as CmpOrdering;
use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use super::search_match::{make_filter, match_indexed_name, prepare_search_name, SearchAlias};
use super::{read_i8, read_string, read_u16, read_u16_or_eof, read_u32, SearchResultItem};
use rotor_platform::file_util;

pub struct FileView {
    pub file_name: String,
    pub path: String,
    pub filter: u32,
    pub rank: i8,
    pub aliases: Option<Box<[String]>>, // Store alias names for this file
    pub search_aliases: Option<Box<[SearchAlias]>>,
}

impl PartialEq for FileView {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank && self.path == other.path && self.file_name == other.file_name
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
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.file_name.cmp(&other.file_name))
    }
}

pub struct FileMap {
    main_set: BTreeSet<FileView>,
}

impl FileMap {
    pub fn new() -> FileMap {
        FileMap {
            main_set: BTreeSet::new(),
        }
    }

    // insert a file to the database by index, file name and parent index
    pub fn insert(&mut self, file_name: String, path: String) {
        let rank = Self::get_file_rank(&file_name);
        let mut aliases = Vec::new();

        // Check if this is an app file and get translation names
        if file_name.ends_with(".app") {
            let app_path = std::path::Path::new(&path).join(&file_name);
            let trans_names = file_util::get_app_trans_names(&app_path);
            if let Ok(names) = trans_names {
                if !names.is_empty() {
                    let names: Vec<String> = names.values().cloned().collect(); // Create aliases from translation names
                    for name in names {
                        if !file_name.contains(&name) {
                            aliases.push(name);
                        }
                    }
                }
            }
        }

        let prepared = prepare_search_name(
            &file_name,
            (!aliases.is_empty()).then_some(aliases.as_slice()),
        );

        self.insert_simple(FileView {
            file_name,
            path,
            filter: prepared.filter,
            rank,
            aliases: (!aliases.is_empty()).then(|| aliases.into_boxed_slice()),
            search_aliases: prepared.aliases,
        });
    }

    // insert a file to the database by index and file struct
    fn insert_simple(&mut self, file: FileView) {
        self.main_set.replace(file);
    }

    // remove item by FileView
    pub fn remove(&mut self, file_name: String, path: String) {
        let rank = Self::get_file_rank(&file_name);
        let file = FileView {
            file_name,
            path,
            filter: 0,
            rank,
            aliases: None,
            search_aliases: None,
        };
        self.main_set.remove(&file);
    }

    // search for files by query
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
                let full_path = std::path::Path::new(&file.path)
                    .join(&file.file_name)
                    .to_string_lossy()
                    .into_owned();
                result.push(SearchResultItem {
                    path: file.path.clone(),
                    file_path: full_path,
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

    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let save_file = fs::File::create(path)?;
        let mut writer = io::BufWriter::new(save_file);
        for file in self.iter() {
            writer.write_all(&(file.file_name.len() as u16).to_be_bytes())?;
            writer.write_all(file.file_name.as_bytes())?;
            writer.write_all(&(file.path.len() as u16).to_be_bytes())?;
            writer.write_all(file.path.as_bytes())?;
            writer.write_all(&file.filter.to_be_bytes())?;
            writer.write_all(&file.rank.to_be_bytes())?;

            // Save aliases count and aliases
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

        while let Some(file_name_len) = read_u16_or_eof(&mut reader)? {
            let file_name = read_string(&mut reader, file_name_len as usize)?;
            let file_path_len = read_u16(&mut reader)?;
            let path = read_string(&mut reader, file_path_len as usize)?;
            let _stored_filter = read_u32(&mut reader)?;
            let rank = read_i8(&mut reader)?;

            // Read aliases count and aliases
            let mut aliases = Vec::new();
            if let Some(aliases_count) = read_u16_or_eof(&mut reader)? {
                for _ in 0..aliases_count {
                    let alias_len = read_u16(&mut reader)?;
                    let alias = read_string(&mut reader, alias_len as usize)?;

                    aliases.push(alias);
                }
            }
            let aliases = (!aliases.is_empty()).then(|| aliases.into_boxed_slice());
            let prepared = prepare_search_name(&file_name, aliases.as_deref());

            self.insert_simple(FileView {
                file_name,
                path,
                filter: prepared.filter,
                rank,
                aliases,
                search_aliases: prepared.aliases,
            });
        }

        Ok(())
    }

    pub fn clear(&mut self) {
        self.main_set.clear();
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

    // return rank by filename
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

    fn search_names(file_map: &FileMap, query: &str) -> Vec<String> {
        let cancel = AtomicBool::new(false);
        let (result, _) = file_map.search(query, 0, 10, &cancel);
        result
            .unwrap_or_default()
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
}
