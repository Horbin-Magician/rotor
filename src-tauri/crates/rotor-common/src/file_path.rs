use std::{env, io, path::PathBuf, sync::OnceLock};

static DATA_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

/// Set once, before configuration, indexes or screenshot records are opened.
pub fn initialize_data_directory(path: PathBuf) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "data directory must be absolute",
        ));
    }
    DATA_DIRECTORY.set(path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "data directory already initialized",
        )
    })
}

pub fn get_tmp_path() -> PathBuf {
    env::temp_dir()
}

pub fn get_userdata_path() -> Option<PathBuf> {
    if let Some(path) = DATA_DIRECTORY.get() {
        return Some(path.clone());
    }
    resolve_data_directory(
        env::var_os("ROTOR_DATA_DIR").map(PathBuf::from),
        env::home_dir(),
        env::current_dir().ok(),
    )
}

fn resolve_data_directory(
    override_path: Option<PathBuf>,
    home: Option<PathBuf>,
    cwd: Option<PathBuf>,
) -> Option<PathBuf> {
    match override_path {
        Some(path) if path.as_os_str().is_empty() => None,
        Some(path) if path.is_absolute() => Some(path),
        Some(path) => cwd.map(|cwd| cwd.join(path)),
        None => home.map(|home| home.join(".rotor")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_data_directory_never_falls_back_to_real_profile() {
        let base = env::temp_dir();
        assert_eq!(
            resolve_data_directory(Some(base.join("isolated")), Some(base.join("home")), None),
            Some(base.join("isolated"))
        );
        assert_eq!(
            resolve_data_directory(
                Some(PathBuf::new()),
                Some(base.join("home")),
                Some(base.clone())
            ),
            None
        );
        assert_eq!(
            resolve_data_directory(Some("relative".into()), None, Some(base.clone())),
            Some(base.join("relative"))
        );
        assert_eq!(
            resolve_data_directory(None, Some(base.clone()), None),
            Some(base.join(".rotor"))
        );
    }
}
