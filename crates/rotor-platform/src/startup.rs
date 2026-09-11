use std::path::Path;

pub const STARTUP_NAME: &str = rotor_common::native_app::STARTUP_NAME;

/// Windows argv quoting, not shell quoting.
pub fn quote_argument(argument: &str) -> Result<String, String> {
    if argument.contains('\0') {
        return Err("Launch argument contains NUL".into());
    }
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for ch in argument.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            quoted.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
            quoted.push(ch);
        } else {
            quoted.extend(std::iter::repeat_n('\\', slashes));
            quoted.push(ch);
        }
        slashes = 0;
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    Ok(quoted)
}

pub fn command_line(executable: &Path, arguments: &[String]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let executable = dunce::simplified(executable);
    let executable = executable
        .to_str()
        .ok_or("Executable path is not Unicode")?;
    let mut parts = vec![quote_argument(executable)?];
    for argument in arguments {
        parts.push(quote_argument(argument)?);
    }
    Ok(parts.join(" "))
}

#[cfg(target_os = "windows")]
pub fn enabled(executable: &Path, arguments: &[String]) -> Result<bool, String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let expected = command_line(executable, arguments)?;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let key = match user.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run") {
        Ok(key) => key,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let value: String = match key.get_value(STARTUP_NAME) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if value != expected {
        return Ok(false);
    }
    if let Ok(approved) = user
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run")
    {
        if let Ok(value) = approved.get_raw_value(STARTUP_NAME) {
            if value.bytes.first() == Some(&3) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

#[cfg(target_os = "windows")]
pub fn set_enabled(enable: bool, executable: &Path, arguments: &[String]) -> Result<(), String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_SET_VALUE, REG_BINARY},
        RegKey, RegValue,
    };
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = user
        .create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        .map_err(|error| error.to_string())?;
    if enable {
        let previous = match key.get_raw_value(STARTUP_NAME) {
            Ok(value) => Some(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        key.set_value(STARTUP_NAME, &command_line(executable, arguments)?)
            .map_err(|error| error.to_string())?;
        if let Ok(approved) = user.open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run",
            KEY_SET_VALUE,
        ) {
            if let Err(error) = approved.set_raw_value(
                STARTUP_NAME,
                &RegValue {
                    vtype: REG_BINARY,
                    bytes: vec![2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                },
            ) {
                let rollback = match previous {
                    Some(value) => key.set_raw_value(STARTUP_NAME, &value),
                    None => key.delete_value(STARTUP_NAME),
                };
                return Err(match rollback {
                    Ok(()) => error.to_string(),
                    Err(rollback) => format!("{error}; startup rollback failed: {rollback}"),
                });
            }
        }
    } else if let Err(error) = key.delete_value(STARTUP_NAME) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn agent_path() -> Result<std::path::PathBuf, String> {
    Ok(std::env::home_dir()
        .ok_or("Home directory is unavailable")?
        .join("Library/LaunchAgents")
        .join(format!(
            "{}.plist",
            rotor_common::native_app::LAUNCH_AGENT_LABEL
        )))
}
#[cfg(target_os = "macos")]
fn agent(executable: &Path, arguments: &[String]) -> Result<plist::Value, String> {
    let mut dictionary = plist::Dictionary::new();
    let mut args = vec![plist::Value::String(
        executable
            .to_str()
            .ok_or("Executable path is not Unicode")?
            .into(),
    )];
    args.extend(arguments.iter().cloned().map(plist::Value::String));
    dictionary.insert(
        "Label".into(),
        plist::Value::String(rotor_common::native_app::LAUNCH_AGENT_LABEL.into()),
    );
    dictionary.insert("ProgramArguments".into(), plist::Value::Array(args));
    dictionary.insert("RunAtLoad".into(), plist::Value::Boolean(true));
    Ok(plist::Value::Dictionary(dictionary))
}
#[cfg(target_os = "macos")]
pub fn enabled(executable: &Path, arguments: &[String]) -> Result<bool, String> {
    let path = agent_path()?;
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    Ok(
        plist::Value::from_reader(std::io::Cursor::new(bytes))
            .map_err(|error| error.to_string())?
            == agent(executable, arguments)?,
    )
}
#[cfg(target_os = "macos")]
pub fn set_enabled(enable: bool, executable: &Path, arguments: &[String]) -> Result<(), String> {
    let path = agent_path()?;
    if enable {
        let mut bytes = Vec::new();
        agent(executable, arguments)?
            .to_writer_xml(&mut bytes)
            .map_err(|error| error.to_string())?;
        rotor_common::persistence::atomic_write_private(&path, &bytes)
            .map_err(|error| error.to_string())?;
    } else if let Err(error) = std::fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(error.to_string());
        }
    }
    Ok(())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn enabled(_: &Path, _: &[String]) -> Result<bool, String> {
    Err("Native startup is unsupported".into())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_enabled(_: bool, _: &Path, _: &[String]) -> Result<(), String> {
    Err("Native startup is unsupported".into())
}

#[cfg(test)]
mod tests {
    use super::quote_argument;
    #[test]
    fn quotes_spaces_embedded_quotes_and_trailing_backslashes() {
        assert_eq!(quote_argument("a b").unwrap(), "\"a b\"");
        assert_eq!(quote_argument("a\"b").unwrap(), "\"a\\\"b\"");
        assert_eq!(
            quote_argument("C:\\folder\\").unwrap(),
            "\"C:\\folder\\\\\""
        );
        assert!(quote_argument("a\0b").is_err());
    }
}
