<p align="center"><img width="100" src="./assets/icons/icon.png" alt="Rotor logo"></p>

# Rotor

A native desktop toolbox built with Rust, GPUI and gpui-component.

[中文](doc/README_CN.md) · [Validation status](doc/validation-status.md) · [Packaging and recovery](native/README.md)

## Features

- Indexed file search with keyboard navigation, exclusions and Windows administrator launch.
- Multi-display screenshots, pinned images, crop/zoom, pen/rectangle/arrow/text annotations, PNG and clipboard export.
- Local Chinese/English OCR using bundled ONNX models; annotations use installed system fonts.
- Input and selection translation with Google, DeepSeek and custom HTTP engines.
- Configurable quick actions, shortcut recording, automatic settings saves, light/dark themes and English/Chinese interfaces.
- Native tray, single-instance handling, startup integration and signed updater verification.

## Current platform status

Release drafts default to Windows x64 and macOS arm64 (macOS 15.0+).
See [validation status](doc/validation-status.md) for executed checks, installation
results and pending manual acceptance. A CI matrix entry is not platform acceptance.

Rotor keeps its version-independent name and `.rotor` production profile path,
with no 2.x import or in-place upgrade. Publishing and stable/preview promotion
follow the [release operations](native/release-operations.md).

Current [native interface screenshots](doc/screenshots/README.md) show search,
annotation, translation and settings using synthetic data.

## Development

Requirements: the Rust toolchain pinned in `rust-toolchain.toml`, MSVC C++ Build Tools and a Windows SDK on Windows. NSIS 3.11 is needed for Windows packaging. Node.js, Yarn, a browser runtime and frontend build commands are not required for native builds.

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

All application and shared crates live under `crates/`; models and icons are under `assets/`. `xtask/` owns versioning, staging, packaging and signature verification; `native/` contains distribution recipes.

The default development identity uses `.rotor-dev` and adds Alt to stored global shortcuts, keeping its namespace separate from the production `.rotor` profile. `--data-dir` or `ROTOR_DATA_DIR` selects an explicit profile. Production identity is enabled with the `production` feature.

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

Use new stage/package directories. For production identity, pass `--production` to both build and stage. Follow [native/README.md](native/README.md) for signatures, native update recovery and silent installation checks.

The root Cargo workspace version is authoritative:

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- set-version 3.0.0 --dry-run
```

After committing `doc/releases/<version>.md` and completing release checks, run
`python3 scripts/bump-version.py 3.0.1` to update the workspace version and lockfile,
commit, tag and push the current branch plus that tag to `origin`. Use `python`
on Windows. `--dry-run` previews the operation; `--no-push` keeps it local.
GitHub Actions creates a draft; publish it manually after review to trigger Gitee
sync. The lower-level `xtask set-version` command still only edits version files.
If the requested version already matches the workspace, the script tags and
pushes the current commit without creating another version commit.

## Contributing and license

Keep business crates free of GPUI, Tauri and WebView dependencies. Use isolated synthetic profiles for tests and preserve user data during failures and recovery.

[MIT License](LICENSE). Text annotations use installed system fonts.
