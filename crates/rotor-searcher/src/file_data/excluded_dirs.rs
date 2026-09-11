use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use rotor_common::AppConfig;

pub const SEARCH_EXCLUDED_DIRS_CONFIG_KEY: &str = "search_excluded_dirs";

#[derive(Clone, Debug, Default)]
pub struct ExcludedDirs {
    names: HashSet<String>,
    paths: Vec<PathBuf>,
}

impl ExcludedDirs {
    pub fn from_config() -> Self {
        let value = AppConfig::lock_global()
            .get(SEARCH_EXCLUDED_DIRS_CONFIG_KEY)
            .cloned()
            .unwrap_or_default();
        parse_excluded_dirs(&value, home_dir().as_deref())
    }

    pub fn is_excluded_name(&self, name: &str) -> bool {
        self.names.contains(&name.to_lowercase())
    }

    pub fn is_excluded_path(&self, path: &Path) -> bool {
        self.matches_configured_path(path) || self.has_excluded_name_component(path)
    }

    #[cfg(target_os = "macos")]
    pub fn is_excluded_parent_path(&self, path: &Path) -> bool {
        path.parent()
            .is_some_and(|parent| self.is_excluded_path(parent))
    }

    fn matches_configured_path(&self, path: &Path) -> bool {
        let normalized_path = normalize_path(path);
        self.paths
            .iter()
            .any(|excluded_path| normalized_path.starts_with(excluded_path))
    }

    fn has_excluded_name_component(&self, path: &Path) -> bool {
        path.components().any(|component| match component {
            Component::Normal(segment) => segment
                .to_str()
                .is_some_and(|segment| self.is_excluded_name(segment)),
            _ => false,
        })
    }
}

fn parse_excluded_dirs(value: &str, home: Option<&Path>) -> ExcludedDirs {
    let mut names = HashSet::new();
    let mut paths = Vec::new();

    for line in value.lines() {
        let entry = line.trim();
        if entry.is_empty() || entry.starts_with('#') {
            continue;
        }

        let expanded_entry = expand_home(entry, home);
        let entry_path = Path::new(&expanded_entry);
        if is_path_entry(entry) {
            paths.push(normalize_path(entry_path));
        } else {
            names.insert(entry.to_lowercase());
        }
    }

    ExcludedDirs { names, paths }
}

fn expand_home(entry: &str, home: Option<&Path>) -> String {
    let Some(home) = home else {
        return entry.to_string();
    };

    if entry == "~" {
        return home.to_string_lossy().into_owned();
    }

    if let Some(rest) = entry
        .strip_prefix("~/")
        .or_else(|| entry.strip_prefix("~\\"))
    {
        return home.join(rest).to_string_lossy().into_owned();
    }

    entry.to_string()
}

fn is_path_entry(entry: &str) -> bool {
    entry.starts_with('~')
        || Path::new(entry).is_absolute()
        || entry.contains('/')
        || entry.contains('\\')
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    normalized
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
impl ExcludedDirs {
    fn parse(value: &str, home: Option<&Path>) -> Self {
        parse_excluded_dirs(value, home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_expands_home_paths() {
        let excluded = ExcludedDirs::parse(
            "~/Library\nnode_modules\n target \n# comment\n",
            Some(Path::new("/Users/alice")),
        );

        assert!(excluded.is_excluded_name("NODE_MODULES"));
        assert!(excluded.is_excluded_name("target"));
        assert!(excluded.is_excluded_path(Path::new("/Users/alice/Library/Caches")));
        assert!(!excluded.is_excluded_path(Path::new("/Users/alice/Documents/Library")));
    }

    #[test]
    fn matches_name_components_anywhere_in_path() {
        let excluded = ExcludedDirs::parse("node_modules\nbuild", None);

        assert!(excluded.is_excluded_path(Path::new("/repo/node_modules/pkg/index.js")));
        assert!(excluded.is_excluded_path(Path::new("/repo/build/app")));
        assert!(!excluded.is_excluded_path(Path::new("/repo/src/app.rs")));
    }
}
