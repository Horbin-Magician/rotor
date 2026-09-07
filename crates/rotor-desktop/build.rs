use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=../../src-tauri/assets/icons/icon.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = |relative: &str| root.join(relative).display().to_string().replace('\\', "/");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = [
        "CARGO_PKG_VERSION_MAJOR",
        "CARGO_PKG_VERSION_MINOR",
        "CARGO_PKG_VERSION_PATCH",
    ]
    .map(|name| {
        env::var(name)
            .unwrap()
            .parse::<u16>()
            .expect("Windows version component exceeds u16")
    });
    let resource = format!(
        r#"
#include <windows.h>
1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Fluctus\0"
      VALUE "FileDescription", "Rotor GPUI Development\0"
      VALUE "FileVersion", "{version}\0"
      VALUE "ProductName", "Rotor GPUI Development\0"
      VALUE "ProductVersion", "{version}\0"
      VALUE "OriginalFilename", "rotor-desktop.exe\0"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x0409, 1200
  END
END
"#,
        icon = path("src-tauri/assets/icons/icon.ico"),
        major = numbers[0],
        minor = numbers[1],
        patch = numbers[2]
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("rotor.rc");
    fs::write(&output, resource).unwrap();
    embed_resource::compile(output, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
