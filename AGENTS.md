# AGENTS.md

This file provides guidance to Codex when working with code in this repository.

## Project Overview

Rotor is a fast, low-occupancy desktop toolbox for Windows and macOS. It is built with Tauri 2, a Rust backend, and a Vue 3 + TypeScript frontend. Current user-facing modules include file search, screenshots, pinned screenshot windows, screenshot OCR, settings/overview, and configurable quick actions.

## Technology Stack

- **Desktop shell**: Tauri 2.
- **Backend**: Rust 2021 with a Cargo workspace under `src-tauri`.
- **Frontend**: Vue 3, TypeScript, Vite 6, Vue Router 4, Vue i18n 11, Naive UI, and Tauri JS APIs.
- **Package management**: Yarn 1.22.22 for frontend/Tauri CLI, Cargo for Rust.
- **Screenshot/OCR**: `xcap`, `image`, `rayon`, and `rust-paddle-ocr` with model assets in `src-tauri/assets/model`.

## Agent Constraints

- Do not enable browser debugging.
- Do not edit generated or dependency output unless the task explicitly requires it: `node_modules`, `dist`, `src-tauri/target`, and generated Tauri schema files under `src-tauri/gen`.
- Keep frontend changes consistent with the existing Vue single-file component style and Naive UI usage.
- Keep backend IPC handlers thin where possible; shared behavior should live in the appropriate workspace crate.

## Development Commands

### Frontend

```bash
# Start Vite on the Tauri dev port, 1420
yarn dev

# Type check and build frontend
yarn build

# Type check only
yarn vue-tsc --noEmit

# Preview the built frontend
yarn preview
```

### Tauri

```bash
# Start the full Tauri development app
yarn tauri dev

# Build the app with the base Tauri config
yarn tauri build

# Tauri automatically merges the matching platform override:
# src-tauri/tauri.macos.conf.json or src-tauri/tauri.windows.conf.json
```

### Rust Workspace

```bash
# Check all Rust crates
cd src-tauri && cargo check --workspace

# Run Rust tests
cd src-tauri && cargo test --workspace

# Build all Rust crates
cd src-tauri && cargo build --workspace
```

## Frontend Structure

- `src/main.ts`: Creates the Vue app and installs router/i18n.
- `src/App.vue`: Wraps pages in Naive UI providers and theme overrides.
- `src/plugins/router.ts`: Defines the Tauri webview routes:
  - `/` -> settings window.
  - `/Searcher` -> search window.
  - `/ScreenShotter/Mask` -> per-monitor screenshot mask windows.
  - `/ScreenShotter/Pin` -> pinned screenshot windows.
- `src/plugins/i18n.ts` and `src/locales/*`: English and Chinese localization.
- `src/shared/api/*`: Shared Tauri IPC and data WebSocket clients.
- `src/features/*`: Typed API wrappers and composables for searcher, screenshot, and quick actions.
- `src/components/setting/*`: Settings, overview, shortcut, quick action, update, and platform titlebar UI.
- `src/components/screenShotter/*`: Screenshot mask, pin canvas, pin toolbar, OCR overlay, drawing, text, and edge UI.
- `src/components/searcher/*`: Search input and result list UI.

## Backend Structure

The backend is a Cargo workspace rooted at `src-tauri/Cargo.toml`.

- `src-tauri/src/lib.rs`: Main Tauri entry point. Registers plugins, IPC commands, global shortcut handler, dock behavior, and app lifecycle.
- `src-tauri/src/command/*`: Tauri command handlers for IPC:
  - `core_cmd.rs`: Config, app version, overview info, shortcut conflict notices, URL opening, and WebSocket port.
  - `quick_cmd.rs`: Quick action CRUD, validation, shortcut registration, rollback, and execution.
  - `screen_shotter_cmd.rs`: Mask/pin commands, save image, persisted pin state, and OCR.
  - `searcher_cmd.rs`: Search requests, index status, open file, and open as admin.
- `src-tauri/crates/rotor-common`: App config, user data paths, and backend i18n.
- `src-tauri/crates/rotor-platform`: Platform-specific file utilities, file icons, permission/status collection, window rects, memory usage, elevation, and open-file behavior.
- `src-tauri/crates/rotor-runtime`: Global application state, tray menu, global shortcut dispatch, quick actions, shortcut conflict notices, and local data WebSocket server.
- `src-tauri/crates/rotor-searcher`: File indexing/search, excluded directory parsing, per-volume search workers, result ranking, and icon attachment.
- `src-tauri/crates/rotor-screenshot`: Monitor capture, mask and pin windows, capture cache, persisted shotter records, image utilities, rectangle detection, and OCR integration.

## Runtime Flows

### Global Shortcuts

- Defaults live in `src-tauri/crates/rotor-common/src/config.rs`.
- macOS defaults: search `Cmd+Shift+F`, screenshot `Cmd+Shift+S`, quick actions `Cmd+Shift+T` and `Cmd+Shift+E`.
- Windows defaults: search `Ctrl+Shift+F`, screenshot `Ctrl+Shift+S`, quick actions `Ctrl+Shift+T` and `Ctrl+Shift+E`.
- Pin window defaults: save `S`, close `Escape`, copy `Enter`, hide `H`.
- `rotor-runtime::handle_global_hotkey_event` dispatches screenshot, searcher, and quick action shortcuts with debounce/stale-press handling.
- Shortcut updates are validated through `tauri-plugin-global-shortcut`; failed registrations emit notices that the settings UI displays.

### Screenshot And Pin Windows

- The screenshot shortcut calls `ScreenShotter::run`, captures all monitors, stores raw RGBA images in `CaptureCache`, and emits `show-mask`.
- Mask windows are labeled `ssmask-*`; pin windows are labeled `sspin-{id}`. The Tauri capability file must allow any new labels.
- Frontend mask and pin pages request image bytes through `src/shared/api/dataSocket.ts`.
- `rotor-runtime::data_server` binds a local WebSocket on `localhost` using the first available port from `10000..=48137`; the frontend gets that port via `get_ws_port`.
- Pinned screenshots persist through `rotor-screenshot::shotter_record`; invalid persisted records are removed during restore.
- OCR uses `img2text` and model files bundled from `src-tauri/assets/model`.

### Search

- `Searcher::new` starts a `FileData` event loop and builds indexes in background workers.
- Showing the search window sends an `Update` message and focuses the hidden `searcher` webview.
- Search results are emitted to the `searcher` window through the `update_result` event.
- Excluded directories are configured by the newline-delimited `search_excluded_dirs` setting. Entries may be directory names or absolute/home-relative paths.
- Windows uses NTFS-specific volume handling when available. macOS indexes the user home, `/Applications`, and `/System/Applications`.

### Quick Actions

- Quick actions are stored in config as a JSON string under `quick_actions` with a `quick_actions_revision`.
- `set_quick_actions` normalizes IDs, names, shortcuts, commands, and enabled state; duplicate enabled shortcuts are rejected.
- On Windows, commands run via `cmd /C`; on other platforms they run via `sh -lc`.
- If shortcut registration fails while saving quick actions, the command rolls back previously registered shortcuts.

## Configuration And Resources

- `src-tauri/tauri.conf.json`: Base Tauri 2 config, updater endpoints, CSP, resources, icons, dev URL, and frontend build commands.
- `src-tauri/tauri.macos.conf.json`: macOS bundle targets and signing override.
- `src-tauri/tauri.windows.conf.json`: Windows NSIS target, WebView bootstrapper, per-machine install mode, and installer languages.
- `src-tauri/capabilities/default.json`: Tauri permissions and allowed window labels.
- User config is persisted as TOML in the app user data directory through `rotor-common::AppConfig`.
- Screenshot OCR models and icon resources are bundled from `src-tauri/assets/**/*`.

## Platform Notes

- Windows startup attempts elevation through `rotor_platform::sys_util::run_as_admin`; file search filters to NTFS drives.
- Windows search and screenshot windows use decoration/taskbar settings tailored for overlay-style utility windows.
- macOS hides the Dock icon with `app.set_dock_visibility(false)` and uses private API/titlebar settings for overlay behavior.
- macOS screenshot behavior depends on screen recording permissions; permission status is surfaced in the settings overview.

## Key Files

- `package.json`: Frontend scripts, dependency versions, and Yarn package manager pin.
- `vite.config.ts`: Vite/Tauri dev-server settings. Port `1420` is strict.
- `src-tauri/Cargo.toml`: Rust workspace and Tauri app dependencies.
- `src-tauri/src/lib.rs`: Tauri builder, plugins, command registration, setup, and run loop.
- `src-tauri/crates/rotor-common/src/config.rs`: Default settings, shortcuts, quick actions, and excluded search directories.
- `src-tauri/crates/rotor-runtime/src/application.rs`: Global app state and shortcut dispatch.
- `src-tauri/crates/rotor-runtime/src/data_server.rs`: Local screenshot data WebSocket.
- `src-tauri/crates/rotor-searcher/src/file_data/mod.rs`: Search index state machine and search task orchestration.
- `src-tauri/crates/rotor-screenshot/src/lib.rs`: Screenshot capture, mask windows, pin windows, and pin restore flow.
