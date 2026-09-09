<p align="center"><img width="100" src="./assets/icons/icon.png" alt="Rotor logo"></p>

# Rotor

A native desktop toolbox built with Rust, GPUI and gpui-component.

[中文](doc/README_CN.md) · [Validation status](doc/validation-status.md) · [Packaging and recovery](native/README.md)

## Features

- Indexed file search with keyboard navigation, exclusions and Windows administrator launch.
- Multi-display screenshots, pinned images, crop/zoom, pen/rectangle/arrow/text annotations, PNG and clipboard export.
- Local Chinese/English OCR using bundled ONNX models and annotation fonts.
- Input and selection translation with Google, DeepSeek and custom HTTP engines.
- Configurable quick actions, shortcut recording, automatic settings saves, light/dark themes and English/Chinese interfaces.
- Native tray, single-instance handling, startup integration and signed updater verification.

## Current platform status

Windows x64 is the active validation target; current local checks use Windows 11. macOS arm64 implementation and packaging remain in the source, with a minimum configured macOS version of 15.0, but macOS validation is deferred. Further visual acceptance tests were explicitly skipped; see the validation status for pending checks.

Published releases and update feeds have not been promoted by the source migration. Build native candidates locally or use the `native-candidate` workflow, which defaults to Windows. The release workflow prepares signed **drafts**; publishing and feed promotion remain separate actions.

## Development

Requirements: the Rust toolchain pinned in `rust-toolchain.toml`, MSVC C++ Build Tools and a Windows SDK on Windows. NSIS 3.11 is needed for Windows packaging. Node.js, Yarn, a browser runtime and frontend build commands are not required for native builds.

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

All application and shared crates live under `crates/`; models, fonts and icons are under `assets/`. `xtask/` owns versioning, staging, packaging and signature verification; `native/` contains distribution recipes.

The default development identity uses `.rotor-gpui` and adds Alt to stored global shortcuts, keeping its namespace separate from the production `.rotor` profile. `--data-dir` or `ROTOR_DATA_DIR` selects an explicit profile. Production identity is enabled with the `production` feature.

Development builds, including `cargo build --release`, fall back to this checkout's `assets/` when no deployed assets are found. This lookup is independent of the working directory. `--resource-dir` or `ROTOR_RESOURCE_DIR` explicitly overrides the resource root. Production builds require deployed assets (or an explicit override) and never fall back to the build checkout; use the staging commands below when distributing the app.

| Action | Stored Windows shortcut | Effective development shortcut |
|---|---|---|
| File search | Ctrl+Shift+F | Ctrl+Alt+Shift+F |
| Screenshot | Ctrl+Shift+S | Ctrl+Alt+Shift+S |
| Selection translation | Ctrl+Shift+D | Ctrl+Alt+Shift+D |
| Input translation | Ctrl+Shift+W | Ctrl+Alt+Shift+W |

Development Settings uses Ctrl+Alt+Shift+G. Pin-local defaults are S to save, Enter to copy, H to hide and Escape to close; text editing has its own confirmation/cancellation behavior.

## Build a native package

```powershell
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
$env:NSIS_MAKENSIS = 'C:/Program Files (x86)/NSIS/makensis.exe'
cargo run -p xtask -- package target/native-stage target/native-package
```

Use new stage/package directories. For production identity, pass `--production` to both build and stage. Follow [native/README.md](native/README.md) for signatures, profile backups, rollback and silent installation checks.

The root Cargo workspace version is authoritative:

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- set-version 2.7.0-beta.1 --dry-run
```

Version changes do not commit, tag or push automatically.

## Contributing and license

Keep business crates free of GPUI, Tauri and WebView dependencies. Use isolated synthetic profiles for tests and preserve user data during failures and recovery.

[MIT License](LICENSE). Bundled Noto Sans CJK includes its OFL license under `assets/fonts/`.
