use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::sync::mpsc::Receiver;

use crate::util::file_util;
use super::SearchResultItem;

#[allow(unused)]
pub struct FileView {
    pub file_name: String,
    pub path: String,
    pub filter: u32,
    pub rank: i8,
    pub aliases: Vec<String>, // Store alias names for this file
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct FileKey {
    rank: i8,
    pub full_name: String,
}

#[allow(unused)]
pub struct FileMap {
    main_map: BTreeMap<FileKey, FileView>,
}

impl FileMap {
    #[allow(unused)]
    pub fn new() -> FileMap {
        FileMap {
            main_map: BTreeMap::new(),
        }
    }

    // insert a file to the database by index, file name and parent index
    #[allow(unused)]
    pub fn insert(&mut self, file_name: String, path: String) {
        let mut filter = make_filter(&file_name);
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
                        if file_name.contains(&name) == false {
                            filter |= make_filter(&name);
                            aliases.push(name);
                        }
                    }
                }
            }
        }

        self.insert_simple(FileView {
            file_name,
            path,
            filter,
            rank,
            aliases,
        });
    }

    // insert a file to the database by index and file struct
    #[allow(unused)]
    fn insert_simple(&mut self, file: FileView) {
        let full_name = format!("{}/{}", file.path, file.file_name);
        let key = FileKey {
            rank: file.rank,
            full_name,
        };
        self.main_map.insert(key, file);
    }

    // remove item by FileView
    #[allow(unused)]
    pub fn remove(&mut self, file_name: String, path: String) {
        let full_name = format!("{}/{}", path, file_name);
        let rank = Self::get_file_rank(&file_name);
        let key = FileKey {
            rank,
            full_name,
        };
        self.main_map.remove(&key);
    }

    // search for files by query
    #[allow(unused)]
    pub fn search(
        &self,
        query: &str,
        last_search_num: usize,
        batch: u8,
        stop_receiver: &Receiver<()>,
    ) -> (Option<Vec<SearchResultItem>>, usize) {
        let mut result = Vec::new();
        let mut find_num = 0;
        let mut search_num: usize = 0;
        let query_lower = query.to_lowercase();
        let query_filter = make_filter(&query_lower);

        let file_map_iter = self.iter().rev().skip(last_search_num);
        for (_, file) in file_map_iter {
            if stop_receiver.try_recv().is_ok() {
                return (None, 0);
            }
            search_num += 1;
            
            // Check if query matches file name or any of its aliases
            let mut is_match = false;
            let mut file_alias = None;
            if (file.filter & query_filter) == query_filter {
                // First check the original file name
                if match_str(&file.file_name, &query_lower) {
                    is_match = true;
                } else {
                    // Then check all aliases
                    for alias in &file.aliases {
                        if match_str(alias, &query_lower) {
                            is_match = true;
                            file_alias = Some(alias.clone());
                            break;
                        }
                    }
                }
            }
            
            if is_match {
                let full_path = format!("{}/{}", file.path, file.file_name);
                let icon_data = file_util::get_file_icon_data(&full_path);

                result.push(SearchResultItem {
                    path: file.path.clone() + "/", // TODO del
                    file_name: file.file_name.clone(),
                    rank: file.rank,
                    icon_data,
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

    #[allow(unused)]
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let mut save_file = fs::File::create(path)?;

        let mut buf = Vec::new();
        for (file_key, file) in self.iter() {
            buf.write_all(&(file.file_name.len() as u16).to_be_bytes())?;
            buf.write_all(file.file_name.as_bytes())?;
            buf.write_all(&(file.path.len() as u16).to_be_bytes())?;
            buf.write_all(file.path.as_bytes())?;
            buf.write_all(&file.filter.to_be_bytes())?;
            buf.write_all(&file.rank.to_be_bytes())?;
            
            // Save aliases count and aliases
            buf.write_all(&(file.aliases.len() as u16).to_be_bytes())?;
            for alias in &file.aliases {
                buf.write_all(&(alias.len() as u16).to_be_bytes())?;
                buf.write_all(alias.as_bytes())?;
            }
        }
        save_file.write_all(&buf.to_vec())?;

        Ok(())
    }

    #[allow(unused)]
    pub fn read(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let file_data = fs::read(path)?;

        let mut ptr_index = 0;
        while ptr_index < file_data.len() {
            if ptr_index + 10 > file_data.len() {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                );
            }

            let file_name_len =
                u16::from_be_bytes(file_data[ptr_index..ptr_index + 2].try_into()?) as u16;
            ptr_index += 2;
            if ptr_index + (file_name_len as usize) + 5 > file_data.len() {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                );
            }
            let file_name = String::from_utf8(
                file_data[ptr_index..(ptr_index + file_name_len as usize)].to_vec(),
            )?;
            ptr_index += file_name_len as usize;

            let file_path_len =
                u16::from_be_bytes(file_data[ptr_index..ptr_index + 2].try_into()?) as u16;
            ptr_index += 2;
            if ptr_index + (file_path_len as usize) + 5 > file_data.len() {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                );
            }
            let path = String::from_utf8(
                file_data[ptr_index..(ptr_index + file_path_len as usize)].to_vec(),
            )?;
            ptr_index += file_path_len as usize;

            let filter = u32::from_be_bytes(file_data[ptr_index..ptr_index + 4].try_into()?);
            ptr_index += 4;
            let rank = i8::from_be_bytes(file_data[ptr_index..ptr_index + 1].try_into()?);
            ptr_index += 1;

            // Read aliases count and aliases
            let mut aliases = Vec::new();
            if ptr_index + 2 <= file_data.len() {
                let aliases_count = u16::from_be_bytes(file_data[ptr_index..ptr_index + 2].try_into()?);
                ptr_index += 2;
                
                for _ in 0..aliases_count {
                    if ptr_index + 2 > file_data.len() {
                        return Err(
                            io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                        );
                    }
                    let alias_len = u16::from_be_bytes(file_data[ptr_index..ptr_index + 2].try_into()?);
                    ptr_index += 2;
                    
                    if ptr_index + (alias_len as usize) > file_data.len() {
                        return Err(
                            io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                        );
                    }
                    let alias = String::from_utf8(
                        file_data[ptr_index..(ptr_index + alias_len as usize)].to_vec(),
                    )?;
                    ptr_index += alias_len as usize;
                    aliases.push(alias);
                }
            }

            self.insert_simple(
                FileView {
                    file_name,
                    path,
                    filter,
                    rank,
                    aliases,
                },
            );
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.main_map.clear();
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
    }

    // get a File by index
    #[allow(unused)]
    fn get(&self, key: FileKey) -> Option<&FileView> {
        return self.main_map.get(&key);
    }

    fn iter(&self) -> std::collections::btree_map::Iter<'_, FileKey, FileView> {
        self.main_map.iter()
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

// Calculates a 32bit value that is used to filter out many files before comparing their filenames
fn make_filter(str: &str) -> u32 {
    /*
    Creates an address that is used to filter out strings that don't contain the queried characters
    Explanation of the meaning of the single bits:
    0-25 a-z
    26 0-9
    27 other ASCII
    28 not in ASCII
    */
    let len = str.len();
    if len == 0 {
        return 0;
    }
    let mut address: u32 = 0;
    let str_lower = str.to_lowercase();

    for c in str_lower.chars() {
        if c == '*' {
            continue; // Reserved for wildcard
        } else if c.is_ascii_lowercase() {
            address |= 1 << (c as u32 - 97);
        } else if c.is_ascii_digit() {
            address |= 1 << 26;
        } else if c < 127u8 as char {
            address |= 1 << 27;
        } else {
            address |= 1 << 28;
        }
    }
    address
}

// return true if contain query
fn match_str(contain: &str, query_lower: &str) -> bool {
    let mut lower_contain = contain.to_lowercase();
    for s in query_lower.split('*') {
        // for wildcard
        if let Some(index) = lower_contain.find(s) {
            lower_contain = (lower_contain.split_at(index).1).to_string();
        } else {
            return false;
        }
    }
    true
}
