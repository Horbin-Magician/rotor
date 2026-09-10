# Native development packages

The root `workspace.package.version` is the sole version authority. Native builds
and `xtask version` do not read a frontend package manifest.

All native and shared crates now live under root `crates/`; bundled models
and icons live under root `assets/`. Native packages retain the installed
`assets/` resource layout. Packaging recipes remain in `native/`.

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
cargo run -p xtask -- verify target/native-stage
$env:NSIS_MAKENSIS = 'C:\path\to\NSIS\makensis.exe'
cargo run -p xtask -- package target/native-stage target/native-package
```

Stage and package destinations must not already exist. Build/stage/package
generate local artifacts; signing uses the separate command below. Staging copies the release
executable, native dynamic libraries, all model/icon assets, native metadata,
and the update public key. `resources.json` records every staged file's size and
SHA-256; packaging verifies it before reading the files. This checksum manifest is
an integrity check, not a signature or trust root.

Windows packages use a separate `Rotor GPUI Development` install directory,
Start menu entry, and uninstall key. The existing `Rotor` installation and user
profile remain separate. Release executables use the GUI subsystem; automation
can still collect exit status and explicitly redirected `--check-config` output.
Windows icon/version are embedded at build time. The pinned GPUI dependency
embeds the per-monitor DPI awareness and as-invoker manifest; do not embed a
second manifest in the application. The application retains its optional elevation
handoff; installer elevation does not change application startup policy.

On macOS arm64, the same commands stage an app bundle and generate `.app.tar.gz`
and `.dmg` files using the host's `tar` and `hdiutil`. Minimum macOS is 15.0.
These development artifacts are not Developer ID signed or notarized. Default
native CI includes macOS compilation, tests and app staging. Quarantine behavior,
signing/notarization, interactive multi-display capture and upgrade installation
remain acceptance gates before release use. Local development requires the pinned
Rust toolchain and Xcode Command Line Tools (`xcode-select --install`).

The native preview feed is `gpui-latest/gpui-latest.json`. It has not been
published. Keep the existing Tauri `latest.json` channel until upgrade/rollback
and both platform acceptance gates pass. The native verifier accepts the same
public key and nested-base64 minisign signatures as the existing Tauri updater.

The Windows update UI re-verifies a downloaded installer while holding it against
write/delete sharing, starts it through the native elevation API, and quits only
after successful launch. NSIS waits up to 30 seconds for the original process to
exit. The finish page can restart with the same data directory and development
shortcut/index/elevation flags. The custom resource-directory override is not
carried into a newly installed package; packaged resource discovery applies.
Actual UAC cancellation, installation, restart, and rollback remain untested.

## Offline profile copies

Close the source application before copying. Import into a new directory, keeping
the original and a separate backup; the command never replaces an existing
profile or backup. Record the source version, or `unknown` if it is unavailable.

```powershell
cargo run -p xtask -- import-profile C:/Users/me/.rotor target/profile-copy target/profile-backup 2.6.0
cargo run -p xtask -- verify-profile target/profile-backup
cargo run -p rotor-desktop -- --data-dir target/profile-copy
```

Only `config.toml` and the complete `shotter` directory are copied, including
other workspaces and unknown fields. Search indexes rebuild. The importer checks
hashes before and after copying, rejects symlinks/nested destinations, retains
backups on validation failure, and publishes the new directory after verification.
Checksums detect changes; they are not a substitute for closing the source app.
`migration-receipt.json` records version/time and file hashes without config
values. Verify immutable backups or a freshly imported copy; ordinary app writes
will legitimately change the active copy's hashes. Rollback imports the selected
backup into another new directory, keeping current data and both backups.

```powershell
cargo run -p rotor-screenshot --example profile_roundtrip -- target/profile-roundtrip
```

This retained synthetic fixture exercises native writes and the legacy Rust
record reader without screenshots or windows. It is not the installed Tauri
client's full upgrade/rollback acceptance test.

The macOS update path prepares a verified app in a private sibling staging
directory before the current app quits. A helper waits for process exit, retains
the previous app, and requires a startup acknowledgement from the replacement.
Startup failure restores and reopens the previous app; failed bundles and update
receipts are retained. It requires write access to the app's parent directory.
This code has portable fixture coverage and Windows type checks; macOS process
control, bundle launch, signing, and real rollback still require Mac validation.

## Versions, updater signatures and candidate CI

```powershell
cargo run -p xtask -- set-version 2.7.0-beta.1 --dry-run
# To edit, run without --dry-run from a clean working tree, then review the diff.
cargo run -p xtask -- sign target/native-package/Rotor-GPUI_2.6.0_x64-setup.exe
cargo run -p xtask -- inventory target/native-package
```

Version editing updates the root workspace and Cargo.lock. It does not commit,
tag or push, and does not require a JavaScript runtime or package manifest. macOS uses numeric Apple version fields and
stores the full preview SemVer in `RotorVersion` for updater comparisons.

`sign` reads the existing `TAURI_SIGNING_PRIVATE_KEY` (base64 value or key-file
path) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` environment variables. It uses
[minisign 0.9.1](https://docs.rs/minisign/0.9.1/minisign/fn.sign.html), writes a
Tauri-compatible base64 `.sig`, and verifies it against the unchanged app public
key before publishing the signature file. Existing valid signatures can be reused.
Signing never prints key material and does not prompt for a password.

```powershell
cargo run -p xtask -- release-manifest merged-artifacts https://github.com/Horbin-Magician/rotor/releases/download/gpui-latest/ notes.txt gpui-latest.json
```

Metadata generation requires both platform archives and their valid signatures.
The manually dispatched `native-candidate` workflow builds development packages
without Node/Yarn and optionally signs them using existing updater secrets. It
uploads review artifacts and preview metadata; it never creates a release or
updates a published feed. Real production signatures and remote CI have not been
run in the migration workspace.

macOS signature validation, Gatekeeper and notarization acceptance remain pending.

## Production-identity acceptance packages

```powershell
cargo run -p xtask -- build --production
cargo run -p xtask -- stage --production target/production-stage
cargo run -p xtask -- package target/production-stage target/production-package
```

Builds save separately hashed binary snapshots for development and production.
Staging requires the matching snapshot and rejects a changed version or flavor.
Use xtask build before staging; a bare cargo build does not create this receipt.
`--build-info` and `--check-resources` are headless diagnostics and return before
profile migration, global shortcuts or windows. The latter checks packaged asset
discovery independently of the current working directory.

Production identity preserves `Rotor`, `rotor.exe`/`rotor`, `cc.fluctus.rotor`, the
old Windows uninstall key/install location, and macOS `Rotor.app`. The first real
production startup holds the old instance namespace and backs up legacy profile
files before native writes. It preserves existing startup choices and uses the
old shortcut values. Production installation and upgrade acceptance remain pending;
these packages have not been promoted to the release channel.

Development uses `gpui-latest.json`; production identity uses
`gpui-production-latest.json`, both under the isolated `gpui-latest` release.
The legacy generic latest feed remains unchanged. The candidate workflow selects
the identity explicitly; metadata generation reads the package's native-build
receipt. Windows accepts both native `/PARENT` handoffs and the old Tauri updater's
`/UPDATE /ARGS` convention. The macOS DMG contains the app and Applications link,
with technical inventories kept outside the user-facing disk image.

Windows installation stages complete files beside the destination and keeps the
previous installation before switching directories. The registered
PreviousInstallLocation is retained for recovery. The initial installer-launched
startup uses the separate `rotor-recovery.exe` launcher, so GUI loader failures
cannot prevent recovery. It waits up to 30 seconds for a startup acknowledgement
and one second of continued process survival. Early exit or timeout invokes the
dedicated rollback entry; it waits
for that process to exit, restores the old directory, keeps the failed new files,
and leaves user data untouched. Ordinary uninstalls never execute the rollback
entry. File/registry permissions, UAC, actual process exit and rollback still need
Windows installation tests; compilation is not an installation acceptance result.

Snapshots also record a normalized digest of native source, Cargo inputs, assets
and installer recipes. Build checks that inputs did not change during compilation;
stage/package reject stale receipts even when the app version is unchanged. Git
is used read-only to enumerate tracked and untracked non-ignored source inputs.
Text line endings are normalized so Windows and macOS candidate receipts agree.

## Silent Windows installation checks

```powershell
./.github/scripts/test-windows-install.ps1 -PackageDirectory target/native-package -TestDirectory D:/path/to/rotor/target/new-install-check
```

This requires PowerShell 7 in an already elevated test session, a development
package and an unused development installation/startup namespace. It checks fresh
installation, locked-file failure, same-version replacement, installed resource
hashes and removal while preserving synthetic user files and backups. The script
uses a new directory under this workspace's target tree and cleans its registry
entries and Start menu shortcut. It does not run the app's visual entry point.

Installers support `/S` and optional `/LOG=<file>` diagnostics. Logs use UTF-16LE
with a BOM so Unicode paths remain readable. Add `-PreviousPackageDirectory` with
a verified lower-version development package to exercise incremental installation.
`-TestRecovery` corrupts only the dedicated test installation's executable and
checks backup restoration plus background restart. `-MeasureIdle` samples that
installation for 120 seconds after 60 seconds of stabilization; indexing and
hotkeys are disabled and the profile is synthetic. These switches do not validate
production identity, old-client update handoff, login restart or the full performance
matrix. The recovery monitor's process fixtures cover exit, hang and acknowledgement;
the actual installed rollback evidence currently covers an invalid executable.
