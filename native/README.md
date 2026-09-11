# Rotor 3 native distribution

Rotor 3 is a fresh Rust + GPUI application. It does not import, detect, upgrade
or remove 2.x profiles or installations. [Validation status](../doc/validation-status.md)
records automated checks, installation results and manual acceptance separately.

## Application identities

| Property | Development | Production |
|---|---|---|
| Product / Windows install directory | Rotor 3 Development | Rotor 3 |
| Bundle ID / Windows registry key | cc.fluctus.rotor3.dev | cc.fluctus.rotor3 |
| Profile directory in the user home | .rotor3-dev | .rotor3 |
| Executable | rotor-desktop | rotor |
| macOS bundle | Rotor 3 Development.app | Rotor 3.app |
| Update channel | preview | stable |

Windows uninstall keys use the registry identifier. Windows startup entries use
the product name; macOS LaunchAgents use the bundle identifier. Single-instance
ownership is scoped to the profile. Runtime and Windows resources share
`native_identity.rs`; tests compare both identities with `native/app.toml`.

`--data-dir` or `ROTOR_DATA_DIR` selects a profile. `--build-info` alone and
`--check-resources` return before profiles, windows, indexing or shortcuts.
`--check-config` tests need a synthetic data directory. Pin persistence uses
`pins/record.toml` and `pins/images/` with required source-image geometry.
Unknown configuration and record keys survive native updates.

## Build, stage and package

Use the pinned Rust toolchain. Windows needs MSVC C++ Build Tools, a Windows SDK
and NSIS 3.11. macOS needs Apple Silicon, macOS 15.0+ and Xcode Command Line Tools.
Node/Yarn are not prerequisites.

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
cargo run -p xtask -- verify target/native-stage
$env:NSIS_MAKENSIS = 'C:/Program Files (x86)/NSIS/makensis.exe'
cargo run -p xtask -- package target/native-stage target/native-package
```

Stage/package destinations must be new. Production passes `--production` to both
build and stage. Release optimization alone does not change identity. A bare
Cargo build does not create the snapshot required by stage. Source/asset changes
require rebuilding.

Resources are independent of the working directory. Production requires deployed
assets or `--resource-dir` / `ROTOR_RESOURCE_DIR`; development may use checkout
assets. `resources.json` covers all staged files with sizes and SHA-256, including
licenses and model provenance. It is an integrity check, not a signature or trust
root. Original third-party licenses are in `licenses/` and beside model assets.

Production Windows uses `Rotor_3.0.0_x64-setup.exe`; development uses `Rotor-Dev`.
macOS produces versioned `.dmg` and `.app.tar.gz` files. The DMG contains the app
and an Applications link. Developer ID signing, notarization and Gatekeeper
acceptance require a Mac and release credentials.

## Signing and release drafts

See [native signing](signing.md) for `ROTOR_SIGNING_*` secrets and key setup.
Only plain prehashed minisign signatures are supported. The workspace version is
authoritative; `set-version 3.0.0 --dry-run` previews an edit. Actual version edits
require a clean working tree and do not commit, tag or push.

```powershell
cargo run -p xtask -- sign target/native-package/Rotor-Dev_3.0.0_x64-setup.exe
cargo run -p xtask -- inventory target/native-package
cargo run -p xtask -- release-manifest target/native-package https://github.com/Horbin-Magician/rotor/releases/download/native-dev-v3.0.0/ doc/releases/3.0.0.md target/native-update.json --platforms windows
```

`--platforms windows|macos|both` is required; missing selected artifacts or
signatures fail. Manifests have `schema_version: 1` and native platform keys.
Artifact URLs name fixed version releases. macOS draft validation requires both
DMG and updater archive signatures.

`native-candidate.yml` creates review artifacts and optional verified metadata.
`publish.yml` builds an existing matching tag, defaults to Windows and macOS,
reads `doc/releases/<version>.md`, checks inventories and manifest agreement,
and creates an unpublished draft with packages, signatures, receipts and
`native-update.json`. Neither workflow promotes channels. See [release operations](release-operations.md).

## Native 3.x recovery and isolated checks

Windows verifies and locks the downloaded installer until launch. NSIS accepts
`/UPDATE` only with `/PARENT=<pid>` and waits for native writes to finish. It stages
files beside the destination and retains the previous install. The separate
`rotor-recovery.exe` monitors restart; a failed start restores the previous 3.x
installation and retains failed files. Profiles are not rolled back.

macOS validates paths, identity and version before replacing the app. Failed
relaunch restores the prior bundle. Portable tests do not prove Mac acceptance.

```powershell
cargo run -p rotor-desktop --example ocr_smoke --release --locked -- --wait-idle
cargo run -p rotor-screenshot --example profile_roundtrip --locked -- target/new-native-roundtrip
./.github/scripts/test-windows-install.ps1 -PackageDirectory target/native-package -TestDirectory D:/Project/rotor/target/new-install-check -TestRecovery
```

The install test requires elevated PowerShell 7, a development package, unused
installation/startup entries and a new target directory. It checks install,
locked-file rejection, replacement, resource discovery, recovery and uninstall,
preserving synthetic profiles. `-PreviousPackageDirectory` needs a verified
lower-version native package. `-MeasureIdle` measures only the test installation.
These do not establish production UAC, login startup, multiscreen/IME or macOS acceptance.
