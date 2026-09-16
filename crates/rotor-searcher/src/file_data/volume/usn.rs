use std::io;

use super::cache::invalid;

/// Decode only the documented V2 layout; never dereference OS-provided offsets.
/// Both enumeration and journal reads start with an eight-byte continuation.
pub(super) struct Records<'a> {
    bytes: &'a [u8],
}

pub(super) struct Record {
    pub index: u64,
    pub parent: u64,
    pub reason: u32,
    pub name: String,
}

pub(super) fn records(bytes: &[u8]) -> io::Result<(u64, Records<'_>)> {
    let header = bytes
        .get(..8)
        .ok_or_else(|| invalid("Truncated USN continuation"))?;
    Ok((
        u64::from_le_bytes(header.try_into().unwrap()),
        Records { bytes: &bytes[8..] },
    ))
}

impl Iterator for Records<'_> {
    type Item = io::Result<Record>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.is_empty() {
            return None;
        }
        let result = decode(self.bytes);
        match result {
            Ok((record, length)) => {
                self.bytes = &self.bytes[length..];
                Some(Ok(record))
            }
            Err(error) => {
                self.bytes = &[];
                Some(Err(error))
            }
        }
    }
}

fn decode(bytes: &[u8]) -> io::Result<(Record, usize)> {
    if bytes.len() < 60 {
        return Err(invalid("Truncated USN record"));
    }
    let length = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    let major = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
    let name_length = u16::from_le_bytes(bytes[56..58].try_into().unwrap()) as usize;
    let name_offset = u16::from_le_bytes(bytes[58..60].try_into().unwrap()) as usize;
    if major != 2
        || length < 60
        || length > bytes.len()
        || name_offset < 60
        || !name_offset.is_multiple_of(2)
        || !name_length.is_multiple_of(2)
        || name_offset + name_length > length
    {
        return Err(invalid("Invalid USN record layout"));
    }
    let name: Vec<_> = bytes[name_offset..name_offset + name_length]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes(pair.try_into().unwrap()))
        .collect();
    Ok((
        Record {
            index: u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            parent: u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            reason: u32::from_le_bytes(bytes[40..44].try_into().unwrap()),
            name: String::from_utf16_lossy(&name),
        },
        length,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_lengths_versions_and_name_offsets() {
        let mut record = vec![0u8; 64];
        record[..4].copy_from_slice(&64u32.to_le_bytes());
        record[4..6].copy_from_slice(&2u16.to_le_bytes());
        record[8..16].copy_from_slice(&42u64.to_le_bytes());
        record[56..58].copy_from_slice(&4u16.to_le_bytes());
        record[58..60].copy_from_slice(&60u16.to_le_bytes());
        record[60..].copy_from_slice(&[b'a', 0, b'b', 0]);
        let mut buffer = 123u64.to_le_bytes().to_vec();
        buffer.extend_from_slice(&record);
        let (next, mut records) = records(&buffer).unwrap();
        assert_eq!(next, 123);
        let parsed = records.next().unwrap().unwrap();
        assert_eq!(parsed.name, "ab");
        assert_eq!(parsed.index, 42);
        assert!(records.next().is_none());
        for length in 1..record.len() {
            assert!(decode(&record[..length]).is_err());
        }
        for (offset, value) in [(0, 0), (0, 255), (4, 3), (56, 3), (58, 59), (58, 64)] {
            let mut invalid = record.clone();
            invalid[offset] = value;
            assert!(decode(&invalid).is_err());
            let mut iter = Records { bytes: &invalid };
            assert!(iter.next().unwrap().is_err());
            assert!(iter.next().is_none());
        }
    }
}
