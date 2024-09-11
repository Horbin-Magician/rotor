use std::collections::{BTreeMap, HashMap};

pub struct File {
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

// Calculates a 32bit value that is used to filter out many files before comparing their filenames
pub fn make_filter(str: &str) -> u32 {
    /*
    Creates an address that is used to filter out strings that don't contain the queried characters
    Explanation of the meaning of the single bits:
    0-25 a-z
    26 0-9
    27 other ASCII
    28 not in ASCII
    */
    let len = str.len();
    if len == 0 { return 0;}
    let mut address: u32 = 0;
    let str_lower = str.to_lowercase();

    for c in str_lower.chars() {
        if c == '*' { 
            continue; // Reserved for wildcard
        } else if c.is_ascii_lowercase() {
            address |= 1 << (c as u32 - 97);
        } else if ('0'..='9').contains(&c) {
            address |= 1 << 26;
        } else if c < 127u8 as char {
            address |= 1 << 27;
        } else {
            address |= 1 << 28;
        }
    }
    address
}

pub struct FileMap {
    main_map: BTreeMap<FileKey, File>,
    rank_map: HashMap<u64, i8, std::hash::BuildHasherDefault<fxhash::FxHasher>>,
}

impl FileMap {
    pub fn new() -> FileMap{
        FileMap {
            main_map: BTreeMap::new(),
            rank_map: HashMap::default(),
        }
    }

    // insert a file to the database by index, file name and parent index
    pub fn insert(&mut self, index: u64, file_name: String, parent_index: u64) {
        let filter = make_filter(&file_name);
        let rank = Self::get_file_rank(&file_name);
        self.insert_simple(index, File { parent_index, file_name, filter, rank });
    }

    // insert a file to the database by index and file struct
    pub fn insert_simple(&mut self, index: u64, file: File) {
        let key = FileKey {
            rank: file.rank,
            index,
        };
        self.rank_map.insert(index, file.rank);
        self.main_map.insert(key, file);
    }

    // get a File by index
    pub fn get(&self, index: &u64) -> Option<&File> {
        if let Some(rank) = self.rank_map.get(index) {
            let file_key = FileKey {
                rank: *rank,
                index: *index,
            };
            return self.main_map.get(&file_key);
        }
        None
    }

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

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, FileKey, File> {
        self.main_map.iter()
    }

    pub fn clear(&mut self) {
        self.main_map.clear();
        self.rank_map.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
    }

    // return rank by filename
    fn get_file_rank(file_name: &str) -> i8 {
        let mut rank: i8 = 0;

        if file_name.to_lowercase().ends_with(".exe") { rank += 10; }
        else if file_name.to_lowercase().ends_with(".lnk") { rank += 25; }

        let tmp = 40i16 - file_name.len() as i16;
        if tmp > 0 { rank += tmp as i8; }

        rank
    }

    // Constructs a path for a directory
    pub fn get_path(&self, index: &u64) -> Option<String> {
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