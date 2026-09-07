use std::{
    env, io,
    path::{Component, Path, PathBuf},
};

/// Installed resources are independent of the working directory and user data.
#[derive(Clone, Debug)]
pub struct ResourceLocator {
    root: PathBuf,
}

impl ResourceLocator {
    pub fn from_root(root: &Path) -> io::Result<Self> {
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "resource root is not a directory",
            ));
        }
        Ok(Self { root })
    }

    pub fn discover(
        executable: &Path,
        override_root: Option<&Path>,
        development_root: Option<&Path>,
    ) -> io::Result<Self> {
        if let Some(root) = override_root {
            return Self::from_root(root);
        }
        let executable_dir = executable.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "executable has no directory")
        })?;
        let mut candidates = vec![executable_dir.join("assets")];
        if executable_dir
            .file_name()
            .is_some_and(|name| name == "MacOS")
        {
            if let Some(contents) = executable_dir.parent() {
                candidates.push(contents.join("Resources/assets"));
            }
        }
        if let Some(root) = development_root {
            candidates.push(root.to_path_buf());
        }
        for candidate in candidates {
            if candidate.is_dir() {
                return Self::from_root(&candidate);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "application assets were not found; set ROTOR_RESOURCE_DIR for a development build",
        ))
    }

    pub fn for_current_process() -> io::Result<Self> {
        let override_root = env::var_os("ROTOR_RESOURCE_DIR").map(PathBuf::from);
        let development = cfg!(debug_assertions)
            .then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"));
        Self::discover(
            &env::current_exe()?,
            override_root.as_deref(),
            development.as_deref(),
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn resolve(&self, relative: &Path) -> io::Result<PathBuf> {
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resource path must stay within assets",
            ));
        }
        let path = self.root.join(relative).canonicalize()?;
        if !path.starts_with(&self.root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resource symlink leaves assets",
            ));
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn installed_layout_and_explicit_override_do_not_depend_on_cwd() {
        let directory = tempfile::tempdir().unwrap();
        let assets = directory.path().join("assets/model");
        fs::create_dir_all(&assets).unwrap();
        fs::write(assets.join("sample.onnx"), b"fixture").unwrap();
        let located =
            ResourceLocator::discover(&directory.path().join("rotor.exe"), None, None).unwrap();
        assert_eq!(
            located.resolve(Path::new("model/sample.onnx")).unwrap(),
            assets.join("sample.onnx").canonicalize().unwrap()
        );
        assert!(located.resolve(Path::new("../outside")).is_err());
        assert!(ResourceLocator::discover(
            &directory.path().join("rotor.exe"),
            Some(&directory.path().join("missing")),
            Some(&directory.path().join("assets"))
        )
        .is_err());
    }

    #[test]
    fn macos_app_bundle_uses_contents_resources() {
        let directory = tempfile::tempdir().unwrap();
        let contents = directory.path().join("Rotor.app/Contents");
        fs::create_dir_all(contents.join("MacOS")).unwrap();
        fs::create_dir_all(contents.join("Resources/assets")).unwrap();
        let located = ResourceLocator::discover(&contents.join("MacOS/rotor"), None, None).unwrap();
        assert_eq!(
            located.root(),
            contents.join("Resources/assets").canonicalize().unwrap()
        );
    }
}
