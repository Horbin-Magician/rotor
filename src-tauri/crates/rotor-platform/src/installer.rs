//! Read VERSIONINFO as data, without loading or executing the installer.
use std::{ffi::c_void, path::Path};
use windows::{
    core::PCWSTR,
    Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW},
};

fn query(buffer: &[u8], name: &str) -> Option<(*mut c_void, u32)> {
    let name = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut pointer = std::ptr::null_mut();
    let mut length = 0;
    unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast(),
            PCWSTR(name.as_ptr()),
            &mut pointer,
            &mut length,
        )
    }
    .as_bool()
    .then_some((pointer, length))
}
fn words(buffer: &[u8], pointer: *mut c_void, count: usize) -> Option<&[u16]> {
    let start = pointer as usize;
    let base = buffer.as_ptr() as usize;
    let end = start.checked_add(count.checked_mul(2)?)?;
    if start < base || end > base.checked_add(buffer.len())? || !start.is_multiple_of(2) {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), count) })
}
fn string(buffer: &[u8], path: &str) -> Option<String> {
    let (pointer, length) = query(buffer, path)?;
    let value = words(buffer, pointer, length as usize)?;
    let end = value
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(value.len());
    String::from_utf16(&value[..end]).ok()
}
pub fn verify(path: &Path, product: &str, version: &str) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let name = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(name.as_ptr()), None) };
    if size == 0 || size > 1024 * 1024 {
        return Err("Installer has no valid version metadata".into());
    }
    let mut buffer = vec![0u8; size as usize];
    unsafe {
        GetFileVersionInfoW(
            PCWSTR(name.as_ptr()),
            None,
            size,
            buffer.as_mut_ptr().cast(),
        )
    }
    .map_err(|e| e.to_string())?;
    let (pointer, length) = query(&buffer, "\\VarFileInfo\\Translation")
        .ok_or("Installer translation table is missing")?;
    if length % 4 != 0 {
        return Err("Invalid installer translation table".into());
    }
    let translations =
        words(&buffer, pointer, length as usize / 2).ok_or("Invalid installer metadata range")?;
    for pair in translations.chunks_exact(2) {
        let prefix = format!("\\StringFileInfo\\{:04x}{:04x}", pair[0], pair[1]);
        if string(&buffer, &format!("{prefix}\\ProductName")).as_deref() == Some(product)
            && string(&buffer, &format!("{prefix}\\ProductVersion")).as_deref() == Some(version)
        {
            return Ok(());
        }
    }
    Err("Installer identity/version differs from the selected update".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn malformed_installers_never_reach_execution() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"not an installer").unwrap();
        assert!(super::verify(file.path(), "Rotor", "2.6.0").is_err());
    }
}
