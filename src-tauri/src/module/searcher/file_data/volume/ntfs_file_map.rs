use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::sync::mpsc::Receiver;

use crate::util::file_util;
use super::SearchResultItem;

#[allow(unused)]
pub struct FileView {
    pub parent_index: u64,
    pub file_name: String,
    pub filter: u32,
    pub rank: i8,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct FileKey {
    rank: i8,
    pub index: u64,
}

#[allow(unused)]
pub struct FileMap {
    pub start_usn: i64,
    main_map: BTreeMap<FileKey, FileView>,
    rank_map: HashMap<u64, i8, std::hash::BuildHasherDefault<fxhash::FxHasher>>,
}

impl FileMap {
    #[allow(unused)]
    pub fn new() -> FileMap {
        FileMap {
            start_usn: 0,
            main_map: BTreeMap::new(),
            rank_map: HashMap::default(),
        }
    }

    // insert a file to the database by index, file name and parent index
    #[allow(unused)]
    pub fn insert(&mut self, index: u64, file_name: String, parent_index: u64) {
        let filter = make_filter(&file_name);
        let rank = Self::get_file_rank(&file_name);
        self.insert_simple(
            index,
            FileView {
                parent_index,
                file_name,
                filter,
                rank,
            },
        );
    }

    // insert a file to the database by index and file struct
    #[allow(unused)]
    fn insert_simple(&mut self, index: u64, file: FileView) {
        let key = FileKey {
            rank: file.rank,
            index,
        };
        self.rank_map.insert(index, file.rank);
        self.main_map.insert(key, file);
    }

    // remove item
    #[allow(unused)]
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
            if (file.filter & query_filter) == query_filter
                && match_str(&file.file_name, &query_lower)
            {
                if let Some(path) = self.get_path(&file.parent_index) {
                    let full_path = format!("{}{}", path, file.file_name);
                    let icon_data = file_util::get_file_icon_data(&full_path);
                    result.push(SearchResultItem {
                        path,
                        file_name: file.file_name.clone(),
                        rank: file.rank,
                        icon_data,
                    });
                    find_num += 1;
                    if find_num >= batch {
                        break;
                    }
                }
            }
        }

        (Some(result), search_num)
    }

    #[allow(unused)]
    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let mut save_file = fs::File::create(path)?;

        let mut buf = Vec::new();
        buf.write_all(&self.start_usn.to_be_bytes())?;
        for (file_key, file) in self.iter() {
            buf.write_all(&file_key.index.to_be_bytes())?;
            buf.write_all(&file.parent_index.to_be_bytes())?;
            buf.write_all(&(file.file_name.len() as u16).to_be_bytes())?;
            buf.write_all(file.file_name.as_bytes())?;
            buf.write_all(&file.filter.to_be_bytes())?;
            buf.write_all(&file.rank.to_be_bytes())?;
        }
        save_file.write_all(&buf.to_vec())?;

        Ok(())
    }

    #[allow(unused)]
    pub fn read(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let file_data = fs::read(path)?;

        if file_data.len() < 8 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "File data too short.").into());
        }

        self.start_usn = i64::from_be_bytes(file_data[0..8].try_into()?);
        let mut ptr_index = 8;

        while ptr_index < file_data.len() {
            if ptr_index + 18 > file_data.len() {
                return Err(
                    io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into(),
                );
            }

            let index = u64::from_be_bytes(file_data[ptr_index..ptr_index + 8].try_into()?);
            ptr_index += 8;
            let parent_index =
                usize::from_be_bytes(file_data[ptr_index..ptr_index + 8].try_into()?) as u64;
            ptr_index += 8;
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
            let filter = u32::from_be_bytes(file_data[ptr_index..ptr_index + 4].try_into()?);
            ptr_index += 4;
            let rank = i8::from_be_bytes(file_data[ptr_index..ptr_index + 1].try_into()?);
            ptr_index += 1;
            self.insert_simple(
                index,
                FileView {
                    parent_index,
                    file_name,
                    filter,
                    rank,
                },
            );
        }

        Ok(())
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.main_map.clear();
        self.rank_map.clear();
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
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

        let tmp = 40i16 - file_name.len() as i16;
        if tmp > 0 {
            rank += tmp as i8;
        }

        rank
    }

    // Constructs a path for a directory
    fn get_path(&self, index: &u64) -> Option<String> {
        let mut path = String::new();
        let mut loop_index = *index;
        while loop_index != 0 {
            let file_op = self.get(&loop_index);
            if let Some(file) = file_op {
                path.insert_str(0, (file.file_name.clone() + "\\").as_str());
                loop_index = file.parent_index;
            } else {
                return None;
            }
        }
        Some(path)
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
