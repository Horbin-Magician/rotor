use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use super::{
    read_i64, read_i8, read_string, read_u16, read_u32, read_u64, read_u64_or_eof, SearchResultItem,
};

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

pub struct FileMap {
    pub start_usn: i64,
    main_map: BTreeMap<FileKey, FileView>,
    rank_map: HashMap<u64, i8, std::hash::BuildHasherDefault<fxhash::FxHasher>>,
}

impl FileMap {
    pub fn new() -> FileMap {
        FileMap {
            start_usn: 0,
            main_map: BTreeMap::new(),
            rank_map: HashMap::default(),
        }
    }

    // insert a file to the database by index, file name and parent index
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
    fn insert_simple(&mut self, index: u64, file: FileView) {
        let key = FileKey {
            rank: file.rank,
            index,
        };
        self.rank_map.insert(index, file.rank);
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
        for (_, file) in file_map_iter {
            if cancel.load(Ordering::Relaxed) {
                return (None, 0);
            }
            search_num += 1;
            if (file.filter & query_filter) == query_filter
                && match_str(&file.file_name, &query_lower)
            {
                if let Some(path) = self.get_path(&file.parent_index) {
                    let full_path = format!("{}{}", path, file.file_name);
                    result.push(SearchResultItem {
                        path,
                        file_path: full_path,
                        file_name: file.file_name.clone(),
                        rank: file.rank,
                        icon_data: None,
                        alias: None,
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

    pub fn save(&self, path: &str) -> Result<(), std::io::Error> {
        let save_file = fs::File::create(path)?;
        let mut writer = io::BufWriter::new(save_file);

        writer.write_all(&self.start_usn.to_be_bytes())?;
        for (file_key, file) in self.iter() {
            writer.write_all(&file_key.index.to_be_bytes())?;
            writer.write_all(&file.parent_index.to_be_bytes())?;
            writer.write_all(&(file.file_name.len() as u16).to_be_bytes())?;
            writer.write_all(file.file_name.as_bytes())?;
            writer.write_all(&file.filter.to_be_bytes())?;
            writer.write_all(&file.rank.to_be_bytes())?;
        }
        writer.flush()?;

        Ok(())
    }

    pub fn read(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let save_file = fs::File::open(path)?;
        let mut reader = io::BufReader::new(save_file);

        self.start_usn = read_i64(&mut reader)?;

        while let Some(index) = read_u64_or_eof(&mut reader)? {
            let parent_index = read_u64(&mut reader)?;
            let file_name_len = read_u16(&mut reader)?;
            let file_name = read_string(&mut reader, file_name_len as usize)?;
            let filter = read_u32(&mut reader)?;
            let rank = read_i8(&mut reader)?;
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

    pub fn clear(&mut self) {
        self.main_map.clear();
        self.rank_map.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
    }

    pub fn len(&self) -> usize {
        self.main_map.len()
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
        let mut segments = Vec::new();
        let mut loop_index = *index;
        while loop_index != 0 {
            let file_op = self.get(&loop_index);
            if let Some(file) = file_op {
                segments.push(file.file_name.as_str());
                loop_index = file.parent_index;
            } else {
                return None;
            }
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
    let lower_contain = contain.to_lowercase();
    let mut offset = 0;
    for s in query_lower.split('*') {
        // for wildcard
        if let Some(index) = lower_contain[offset..].find(s) {
            offset += index + s.len();
        } else {
            return false;
        }
    }
    true
}
