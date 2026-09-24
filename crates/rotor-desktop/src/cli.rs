//! Command-line contract of the desktop executable.
//!
//! Parsing lives here so the process roles (diagnostics, update helper,
//! elevated relaunch, normal start) are testable without a display.
use rotor_common::startup_flags;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

const DIAGNOSTIC_FLAGS: [&str; 5] = [
    "--check-config",
    "--check-resources",
    "--build-info",
    "--apply-update",
    "--update-ready",
];

pub(crate) struct Arguments {
    values: Vec<String>,
}

impl Arguments {
    pub(crate) fn from_env() -> Self {
        Self::new(std::env::args().collect())
    }

    pub(crate) fn new(values: Vec<String>) -> Self {
        Self { values }
    }

    pub(crate) fn has(&self, flag: &str) -> bool {
        self.values.iter().any(|value| value == flag)
    }

    /// The value following `--key`; a missing, empty or flag-like value is an error.
    pub(crate) fn value(&self, key: &str) -> Result<Option<String>, String> {
        let Some(index) = self.values.iter().position(|value| value == key) else {
            return Ok(None);
        };
        self.values
            .get(index + 1)
            .filter(|value| !value.starts_with("--") && !value.is_empty())
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("Missing value for {key}"))
    }

    /// `--build-info` must be the only argument.
    pub(crate) fn is_build_info(&self) -> bool {
        self.is_exactly(&["--build-info"])
    }

    /// True when the arguments after the executable are exactly `expected`.
    #[warn(dead_code)]
    pub(crate) fn is_exactly(&self, expected: &[&str]) -> bool {
        self.values.len() == expected.len() + 1 && self.values[1..] == expected[..]
    }

    /// Modes that print or hand off and must never show a startup dialog.
    pub(crate) fn is_diagnostic(&self) -> bool {
        self.values
            .iter()
            .any(|value| DIAGNOSTIC_FLAGS.contains(&value.as_str()))
    }

    pub(crate) fn runtime_flags(&self) -> Vec<String> {
        startup_flags::runtime_flags(&self.values)
    }

    /// Arguments for relaunching elevated: the resolved profile and resource
    /// roots come first, every other argument follows, and a stale
    /// `--wait-for-instance` is replaced by a fresh one.
    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn elevated_relaunch(
        &self,
        directory: &Path,
        resources: Option<&Path>,
    ) -> Vec<String> {
        let mut forwarded = vec![
            "--wait-for-instance".into(),
            "--data-dir".into(),
            directory.to_string_lossy().into_owned(),
        ];
        if let Some(resources) = resources {
            forwarded.extend([
                "--resource-dir".into(),
                resources.to_string_lossy().into_owned(),
            ]);
        }
        let mut index = 1;
        while index < self.values.len() {
            if matches!(self.values[index].as_str(), "--data-dir" | "--resource-dir") {
                index += 2;
                continue;
            }
            if self.values[index] != "--wait-for-instance" {
                forwarded.push(self.values[index].clone());
            }
            index += 1;
        }
        forwarded
    }
}

/// The profile directory: an explicit value (argument or `ROTOR_DATA_DIR`)
/// resolved against the working directory, otherwise the identity's
/// directory in the home folder. An explicit empty value is an error rather
/// than a silent fallback to the real profile.
pub(crate) fn resolve_data_directory(
    explicit: Option<OsString>,
    home: Option<PathBuf>,
    working_directory: Option<PathBuf>,
) -> Result<PathBuf, String> {
    match explicit {
        Some(value) if value.is_empty() => Err("ROTOR_DATA_DIR cannot be empty".into()),
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                Ok(path)
            } else {
                Ok(working_directory
                    .ok_or("working directory unavailable")?
                    .join(path))
            }
        }
        None => Ok(home
            .ok_or("home directory unavailable")?
            .join(rotor_common::native_app::PROFILE_DIRECTORY)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Arguments {
        Arguments::new(
            std::iter::once("rotor")
                .chain(values.iter().copied())
                .map(String::from)
                .collect(),
        )
    }

    #[test]
    fn values_require_a_real_operand() {
        let args = arguments(&["--data-dir", "/tmp/p", "--resource-dir", "--no-index"]);
        assert_eq!(args.value("--data-dir").unwrap().as_deref(), Some("/tmp/p"));
        assert!(args.value("--resource-dir").is_err());
        assert_eq!(args.value("--missing").unwrap(), None);
        assert!(args.has("--no-index"));
        assert_eq!(args.runtime_flags(), vec!["--no-index".to_string()]);
        assert!(!args.is_diagnostic());
        assert!(arguments(&["--check-config"]).is_diagnostic());
        assert!(arguments(&["--build-info"]).is_build_info());
        assert!(!arguments(&["--build-info", "--no-index"]).is_build_info());
        assert!(
            arguments(&["--apply-update", "/tmp/job"]).is_exactly(&["--apply-update", "/tmp/job"])
        );
        assert!(
            !arguments(&["--apply-update", "/tmp/job", "x"])
                .is_exactly(&["--apply-update", "/tmp/job"])
        );
    }

    #[test]
    fn elevated_relaunch_pins_roots_and_forwards_the_rest_once() {
        let args = arguments(&[
            "--wait-for-instance",
            "--data-dir",
            "relative",
            "--no-hotkeys",
            "--resource-dir",
            "old",
            "--background",
        ]);
        assert_eq!(
            args.elevated_relaunch(Path::new("/profiles/p"), Some(Path::new("/assets"))),
            [
                "--wait-for-instance",
                "--data-dir",
                "/profiles/p",
                "--resource-dir",
                "/assets",
                "--no-hotkeys",
                "--background",
            ]
            .map(String::from)
        );
        assert_eq!(
            arguments(&[]).elevated_relaunch(Path::new("/p"), None),
            ["--wait-for-instance", "--data-dir", "/p"].map(String::from)
        );
    }

    #[test]
    fn data_directory_resolution_never_silently_uses_the_real_profile() {
        let home = Some(PathBuf::from("/home/user"));
        let cwd = Some(PathBuf::from("/work"));
        assert!(resolve_data_directory(Some(OsString::new()), home.clone(), cwd.clone()).is_err());
        assert_eq!(
            resolve_data_directory(Some("/abs".into()), home.clone(), cwd.clone()).unwrap(),
            PathBuf::from("/abs")
        );
        assert_eq!(
            resolve_data_directory(Some("rel".into()), home.clone(), cwd).unwrap(),
            PathBuf::from("/work/rel")
        );
        assert!(resolve_data_directory(Some("rel".into()), home.clone(), None).is_err());
        assert_eq!(
            resolve_data_directory(None, home, None).unwrap(),
            PathBuf::from("/home/user").join(rotor_common::native_app::PROFILE_DIRECTORY)
        );
        assert!(resolve_data_directory(None, None, None).is_err());
    }
}
