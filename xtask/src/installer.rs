use crate::{files, Result};
use std::{collections::BTreeSet, path::Path};

pub fn uninstall_script(stage: &Path, executable: &str) -> Result<String> {
    let mut paths = Vec::new();
    files(stage, &mut paths)?;
    let mut directories = BTreeSet::new();
    let mut script = String::new();
    for path in paths {
        let relative = path.strip_prefix(stage)?;
        let name = relative
            .to_str()
            .ok_or("Installer resource path is not Unicode")?;
        if name.contains(['"', '\r', '\n']) {
            return Err("Unsupported installer resource name".into());
        }
        if name != executable {
            script.push_str(&format!(
                "Delete \"$INSTDIR\\{}\"\n",
                name.replace('/', "\\").replace('$', "$$")
            ));
        }
        let mut parent = relative.parent();
        while let Some(path) = parent.filter(|path| !path.as_os_str().is_empty()) {
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    let mut directories = directories.into_iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in directories {
        // Non-recursive removal leaves user-added files and directories intact.
        script.push_str(&format!(
            "RMDir \"$INSTDIR\\{}\"\n",
            path.to_string_lossy().replace('/', "\\").replace('$', "$$")
        ));
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    #[test]
    fn uninstall_only_names_packaged_files_and_escapes_nsis_variables() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("assets/models")).unwrap();
        std::fs::write(
            directory.path().join("assets/models/$model.onnx"),
            b"fixture",
        )
        .unwrap();
        std::fs::write(directory.path().join("rotor-desktop.exe"), b"fixture").unwrap();
        let script = super::uninstall_script(directory.path(), "rotor-desktop.exe").unwrap();
        assert!(script.contains("Delete \"$INSTDIR\\assets\\models\\$$model.onnx\""));
        assert!(!script.contains("rotor-desktop.exe"));
        assert!(!script.contains("/r"));
        assert!(
            script.find("RMDir \"$INSTDIR\\assets\\models\"").unwrap()
                < script.find("RMDir \"$INSTDIR\\assets\"").unwrap()
        );
    }
}
