use std::{env, path::PathBuf};

pub fn get_tmp_path() -> PathBuf {
    env::temp_dir()
}

pub fn get_userdata_path() -> Option<PathBuf> {
    env::home_dir().map(|home_path| home_path.join(".rotor"))
}
