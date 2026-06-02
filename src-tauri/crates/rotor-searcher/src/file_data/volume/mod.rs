pub mod default_file_map;
pub mod default_volume;
mod search_match;

use std::fs::Metadata;
use std::io::{self, Read};
use std::time::UNIX_EPOCH;

#[cfg(target_os = "windows")]
pub mod ntfs_file_map;
#[cfg(target_os = "windows")]
pub mod ntfs_volume;

#[derive(serde::Serialize)]
pub struct SearchResultItem {
    pub path: String,
    pub file_path: String,
    pub file_name: String,
    pub rank: i8,
    pub icon_data: Option<String>, // Base64 encoded icon data
    pub alias: Option<String>,
}

impl Clone for SearchResultItem {
    fn clone(&self) -> Self {
        SearchResultItem {
            path: self.path.clone(),
            file_path: self.file_path.clone(),
            file_name: self.file_name.clone(),
            rank: self.rank,
            icon_data: self.icon_data.clone(),
            alias: self.alias.clone(),
        }
    }
}

impl SearchResultItem {
    pub fn attach_icon_data(mut self) -> Self {
        self.icon_data = rotor_platform::file_util::get_file_icon_data(&self.file_path);
        self
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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

pub fn index_file_stem(name: &str) -> String {
    let stem = name
        .trim_start_matches('/')
        .replace('/', "_")
        .replace('\\', "_");

    if stem.is_empty() {
        "root".to_string()
    } else {
        stem
    }
}

pub(super) fn read_u16_or_eof(reader: &mut impl Read) -> io::Result<Option<u16>> {
    let mut bytes = [0u8; 2];
    match reader.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(u16::from_be_bytes(bytes))),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(error),
    }
}

pub(super) fn read_u16(reader: &mut impl Read) -> io::Result<u16> {
    read_u16_or_eof(reader)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected end of index"))
}

pub(super) fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

#[cfg(target_os = "windows")]
pub(super) fn read_u64_or_eof(reader: &mut impl Read) -> io::Result<Option<u64>> {
    let mut bytes = [0u8; 8];
    match reader.read_exact(&mut bytes) {
        Ok(()) => Ok(Some(u64::from_be_bytes(bytes))),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn read_u64(reader: &mut impl Read) -> io::Result<u64> {
    read_u64_or_eof(reader)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Unexpected end of index"))
}

#[cfg(target_os = "windows")]
pub(super) fn read_i64(reader: &mut impl Read) -> io::Result<i64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(i64::from_be_bytes(bytes))
}

pub(super) fn read_i8(reader: &mut impl Read) -> io::Result<i8> {
    let mut bytes = [0u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(i8::from_be_bytes(bytes))
}

pub(super) fn read_string(reader: &mut impl Read, len: usize) -> io::Result<String> {
    let mut bytes = vec![0u8; len];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
