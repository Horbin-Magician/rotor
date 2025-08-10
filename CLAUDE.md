# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rotor is a fast, low-occupancy toolbox for Windows and macOS built with Tauri. It combines a Rust backend with a Vue.js frontend to provide system-level functionality including file search and screenshot capabilities.

## Architecture

### Technology Stack
- **Backend**: Rust with Tauri framework
- **Frontend**: Vue 3 + TypeScript with Vite
- **Package Management**: Yarn (frontend), Cargo (backend)

### Key Components

**Frontend (`/src/`)**:
- Vue 3 application with TypeScript
- Vue Router for navigation between modules
- Vue i18n for internationalization (English/Chinese)
- Components organized by functionality (ScreenShotter, Searcher, Settings)

**Backend (`/src-tauri/src/`)**:
- Modular Rust architecture with clear separation of concerns
- **Commands** (`/command/`): Tauri command handlers for frontend-backend communication
  - `config_cmd.rs`: Configuration management
  - `screen_shotter_cmd.rs`: Screenshot functionality
  - `searcher_cmd.rs`: File search operations
- **Core** (`/core/`): Application core logic and configuration
- **Modules** (`/module/`): Feature implementations
  - `searcher/`: File indexing and search with volume-specific optimizations
  - `screen_shotter/`: Screen capture functionality
  - `tray/`: System tray integration
- **Utilities** (`/util/`): Cross-platform system utilities

## Development Commands

### Frontend Development
```bash
# Start development server
yarn dev

# Build frontend for production
yarn build

# Type check
vue-tsc --noEmit

# Preview production build
yarn preview
```

### Tauri Development
```bash
# Start Tauri development mode (runs both frontend and backend)
yarn tauri dev

# Build application for distribution
yarn tauri build

# Run Tauri CLI commands
yarn tauri [command]
```

### Rust Backend
```bash
# Build Rust backend
cd src-tauri && cargo build

# Run tests
cd src-tauri && cargo test

# Check for compilation errors
cd src-tauri && cargo check
```

## Project Structure Notes

- Configuration files use TOML format for Rust components and JSON for frontend
- The searcher module includes platform-specific optimizations (NTFS volume support for Windows)
- Global shortcuts are implemented: `Shift+F` for search, `Shift+C` for screenshot
- The application runs in system tray mode with dock visibility disabled on macOS
- Frontend and backend communicate through Tauri's IPC system using generated handlers

## Platform-Specific Considerations

- **Windows**: Uses NTFS-specific file indexing and USN journal watching
- **macOS**: Dock visibility is disabled, runs as menu bar application
- Cross-platform screen capture and file system operations are abstracted in utility modules

## Key Files for Development

- `src-tauri/src/lib.rs`: Main application entry point and plugin configuration
- `src-tauri/tauri.conf.json`: Tauri application configuration
- `package.json`: Frontend dependencies and build scripts
- `src-tauri/Cargo.toml`: Rust dependencies and build configuration