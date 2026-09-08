use std::{env, io, path::PathBuf, sync::OnceLock};

static DATA_DIRECTORY: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Set once, before configuration, indexes or screenshot records are opened.
pub fn initialize_data_directory(path: PathBuf) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "data directory must be absolute",
        ));
    }
    DATA_DIRECTORY.set(Some(path)).map_err(|_| {
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
    DATA_DIRECTORY
        .get_or_init(|| {
            resolve_data_directory(
                env::var_os("ROTOR_DATA_DIR").map(PathBuf::from),
                env::home_dir(),
                env::current_dir().ok(),
            )
        })
        .clone()
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
    fn data_directory_cannot_change_after_its_first_use() {
        let original = get_userdata_path();
        assert!(initialize_data_directory(env::temp_dir().join("too-late")).is_err());
        assert_eq!(get_userdata_path(), original);
    }

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
