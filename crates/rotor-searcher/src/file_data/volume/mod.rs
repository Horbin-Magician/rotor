mod cache;
#[cfg(any(target_os = "macos", test))]
pub mod default_file_map;
#[cfg(any(target_os = "macos", test))]
pub mod default_volume;
#[cfg(test)]
mod release_tests;
mod search_match;

use std::fs::Metadata;
use std::io::{self, Read};
use std::time::UNIX_EPOCH;

#[cfg(target_os = "windows")]
pub mod ntfs_file_map;
#[cfg(target_os = "windows")]
pub mod ntfs_volume;
#[cfg(target_os = "windows")]
mod usn;

/// A position in a volume's descending index order. Owned by the coordinator,
/// so a cancelled worker cannot advance the next accepted request's position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchCursor {
    pub rank: i8,
    pub id: u64,
    pub name: String,
}

pub struct SearchPage {
    pub items: Vec<SearchResultItem>,
    pub cursor: Option<SearchCursor>,
    pub exhausted: bool,
}

#[derive(Clone)]
pub struct SearchResultItem {
    pub path: String,
    pub file_path: String,
    pub file_name: String,
    pub rank: i8,
    pub alias: Option<String>,
}

#[derive(Clone, Debug)]

pub struct VolumeIndexStatus {
    pub name: String,
    pub indexed: bool,
    pub index_item_count: Option<usize>,
    pub index_file_size_bytes: u64,
    pub index_file_modified_at: Option<u64>,
}

pub fn metadata_modified_at(metadata: &Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
}

pub(super) fn read_u16(reader: &mut impl Read) -> io::Result<u16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_be_bytes(bytes))
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

#[cfg(any(target_os = "macos", test))]
pub(super) fn read_u8(reader: &mut impl Read) -> io::Result<u8> {
    let mut bytes = [0u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(u8::from_be_bytes(bytes))
}

#[cfg(target_os = "windows")]
pub(super) fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_be_bytes(bytes))
}

#[cfg(target_os = "windows")]
pub(super) fn read_i64(reader: &mut impl Read) -> io::Result<i64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(i64::from_be_bytes(bytes))
}

pub(super) fn read_string(reader: &mut impl Read, len: usize) -> io::Result<String> {
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
