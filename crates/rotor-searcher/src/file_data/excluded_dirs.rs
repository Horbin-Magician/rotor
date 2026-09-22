use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

#[cfg(not(test))]
use rotor_common::AppConfig;

#[cfg(not(test))]
pub const SEARCH_EXCLUDED_DIRS_CONFIG_KEY: &str = "search_excluded_dirs";

#[derive(Clone, Debug, Default)]
pub struct ExcludedDirs {
    names: HashSet<String>,
    paths: Vec<PathBuf>,
}

impl ExcludedDirs {
    pub(super) fn cache_identity(&self) -> String {
        let mut names: Vec<_> = self.names.iter().map(String::as_str).collect();
        let mut paths: Vec<_> = self
            .paths
            .iter()
            .map(|path| path.to_string_lossy())
            .collect();
        names.sort_unstable();
        paths.sort_unstable();
        format!("{names:?}\n{paths:?}")
    }
    #[cfg(not(test))]
    pub fn from_config() -> Self {
        let value = AppConfig::lock_global()
            .get(SEARCH_EXCLUDED_DIRS_CONFIG_KEY)
            .cloned()
            .unwrap_or_default();
        parse_excluded_dirs(&value, home_dir().as_deref())
    }

    pub fn is_excluded_name(&self, name: &str) -> bool {
        name.eq_ignore_ascii_case(".search-index") || self.names.contains(&name.to_lowercase())
    }

    pub fn is_excluded_path(&self, path: &Path) -> bool {
        self.matches_configured_path(path) || self.has_excluded_name_component(path)
    }

    #[cfg(any(target_os = "macos", test))]
    pub fn is_excluded_parent_path(&self, path: &Path) -> bool {
        path.parent()
            .is_some_and(|parent| self.is_excluded_path(parent))
    }

    fn matches_configured_path(&self, path: &Path) -> bool {
        if self.paths.is_empty() {
            return false;
        }
        let normalized_path = normalize_path(path);
        self.paths
            .iter()
            .any(|excluded_path| path_starts_with(&normalized_path, excluded_path))
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

pub(super) fn parse_excluded_dirs(value: &str, home: Option<&Path>) -> ExcludedDirs {
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

/// Component-wise prefix test. Windows volumes are case-insensitive and NTFS
/// results carry the on-disk casing, so compare components ignoring case there.
fn path_starts_with(path: &Path, prefix: &Path) -> bool {
    #[cfg(not(target_os = "windows"))]
    {
        path.starts_with(prefix)
    }
    #[cfg(target_os = "windows")]
    {
        let mut components = path.components();
        prefix.components().all(|expected| {
            components.next().is_some_and(|actual| {
                actual.as_os_str().to_string_lossy().to_lowercase()
                    == expected.as_os_str().to_string_lossy().to_lowercase()
            })
        })
    }
}

/// `HOME` is normally unset on Windows; the standard resolver also reads `USERPROFILE`.
fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
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

    // std::path only treats backslashes as separators on Windows.
    #[cfg(target_os = "windows")]
    #[test]
    fn expands_home_with_windows_separators() {
        let excluded =
            ExcludedDirs::parse("~\\AppData\\Local\n~", Some(Path::new("C:\\Users\\alice")));

        assert!(excluded.is_excluded_path(Path::new("C:\\Users\\alice\\AppData\\Local\\Temp")));
        assert!(excluded.is_excluded_path(Path::new("C:\\Users\\alice\\Documents")));
        assert!(!excluded.is_excluded_path(Path::new("C:\\Users\\bob\\Documents")));
    }

    #[test]
    fn home_directory_resolves_without_home_variable() {
        // Windows sets USERPROFILE rather than HOME; either must be enough.
        let configured = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"));
        assert_eq!(home_dir().is_some(), configured.is_some());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn configured_paths_ignore_case_on_windows() {
        let excluded = ExcludedDirs::parse("C:\\Users\\Alice\\Downloads", None);

        assert!(excluded.is_excluded_path(Path::new("c:\\users\\alice\\downloads\\setup.exe")));
        assert!(excluded.is_excluded_path(Path::new("C:\\USERS\\ALICE\\DOWNLOADS")));
        assert!(!excluded.is_excluded_path(Path::new("C:\\Users\\Alice\\Downloads2\\file")));
        assert!(!excluded.is_excluded_path(Path::new("D:\\Users\\Alice\\Downloads\\file")));
    }

    #[test]
    fn matches_name_components_anywhere_in_path() {
        let excluded = ExcludedDirs::parse("node_modules\nbuild", None);

        assert!(excluded.is_excluded_path(Path::new("/repo/node_modules/pkg/index.js")));
        assert!(excluded.is_excluded_path(Path::new("/repo/build/app")));
        assert!(!excluded.is_excluded_path(Path::new("/repo/src/app.rs")));
    }
}
