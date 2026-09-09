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
    pub fn verify_native_resources(&self) -> io::Result<()> {
        for name in [
            "model/pp-ocrv6_tiny_det.onnx",
            "model/pp-ocrv6_tiny_rec.onnx",
            "model/ppocrv6_tiny_dict.txt",
            "fonts/NotoSansCJKsc-Regular.otf",
            "fonts/LICENSE-NotoSansCJK.txt",
        ] {
            let path = self.resolve(Path::new(name))?;
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Missing or empty native resource: {name}"),
                ));
            }
        }
        Ok(())
    }
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
        let workspace_assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        Self::discover_for_build(
            &env::current_exe()?,
            override_root.as_deref(),
            &workspace_assets,
        )
    }

    fn discover_for_build(
        executable: &Path,
        override_root: Option<&Path>,
        workspace_assets: &Path,
    ) -> io::Result<Self> {
        // Optimization does not change the app's identity: local release builds
        // still use development resources. Production must use deployed assets.
        Self::discover(
            executable,
            override_root,
            (!crate::native_app::PRODUCTION).then_some(workspace_assets),
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
    fn source_fallback_follows_app_identity_including_release_builds() {
        let directory = tempfile::tempdir().unwrap();
        let workspace_assets = directory.path().join("workspace/assets");
        let executable = directory.path().join("output/release/rotor.exe");
        fs::create_dir_all(workspace_assets.join("fonts")).unwrap();
        fs::write(workspace_assets.join("fonts/sample.otf"), b"fixture").unwrap();

        let result = ResourceLocator::discover_for_build(&executable, None, &workspace_assets);
        if crate::native_app::PRODUCTION {
            assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
        } else {
            let located = result.unwrap();
            assert_eq!(
                located.resolve(Path::new("fonts/sample.otf")).unwrap(),
                workspace_assets
                    .join("fonts/sample.otf")
                    .canonicalize()
                    .unwrap()
            );
        }
    }

    #[test]
    fn deployed_assets_and_explicit_overrides_precede_source_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let workspace_assets = directory.path().join("workspace/assets");
        let deployed_assets = directory.path().join("install/assets");
        let override_assets = directory.path().join("custom/assets");
        let executable = directory.path().join("install/rotor.exe");
        for path in [&workspace_assets, &deployed_assets, &override_assets] {
            fs::create_dir_all(path).unwrap();
        }
        let located =
            ResourceLocator::discover_for_build(&executable, None, &workspace_assets).unwrap();
        assert_eq!(located.root(), deployed_assets.canonicalize().unwrap());
        let located = ResourceLocator::discover_for_build(
            &executable,
            Some(&override_assets),
            &workspace_assets,
        )
        .unwrap();
        assert_eq!(located.root(), override_assets.canonicalize().unwrap());
        assert!(ResourceLocator::discover_for_build(
            &executable,
            Some(&directory.path().join("missing")),
            &workspace_assets,
        )
        .is_err());
    }

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
