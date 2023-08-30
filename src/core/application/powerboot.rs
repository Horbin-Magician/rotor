use std::io;
use std::env;
use winreg::enums::*;
use winreg::RegKey;

pub struct PowerBoot;

impl PowerBoot {
    pub fn set_process_auto_run() -> io::Result<()> {
        let programe_key = "Rotor";
        let file_path = env::current_exe()?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_ALL_ACCESS)?;

        // if let Ok(data) = key.get_value::<String, &str>(programe_key) {
        //     if data == file_path.to_str().unwrap() {
        //         return Ok(());
        //     }
        // }

        key.set_value(programe_key, &file_path.to_str().unwrap())?;

        // key.delete_value(programe_key)?;

        Ok(())
    }
}