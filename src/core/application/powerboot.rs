use std::io;
use std::env;
use winreg::enums::*;
use winreg::RegKey;

pub struct PowerBoot;

impl PowerBoot {
    pub fn set_power_boot(if_power_boot: bool) -> io::Result<()> {
        let programe_key = "Rotor";
        let file_path = env::current_exe()?;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_ALL_ACCESS)?;

        if if_power_boot {
            key.set_value(programe_key, &file_path.to_str().unwrap())?;
        } else {
            key.delete_value(programe_key)?;
        }
        Ok(())
    }
}