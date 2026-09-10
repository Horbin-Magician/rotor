<p align="center"><img width="100" src="../assets/icons/icon.png" alt="Rotor logo"></p>

# Rotor

使用 Rust、GPUI 和 gpui-component 构建的原生桌面工具箱。

[English](../README.md) · [验收状态](validation-status.md) · [打包与恢复](../native/README.md)

## 功能

- 文件索引搜索、键盘导航、目录排除及 Windows 管理员打开。
- 多屏截图、贴图、裁剪缩放、画笔/矩形/箭头/文字标注、PNG 与剪贴板导出。
- 使用内置 ONNX 模型和字体的本地中英文 OCR。
- 输入及划词翻译，支持 Google、DeepSeek 和自定义 HTTP 引擎。
- 快捷操作、快捷键录制、设置自动保存、中英文及浅色/深色主题。
- 原生托盘、单实例、自启动与更新签名校验。

## 当前平台状态

默认原生 CI 已覆盖 Windows x64 和 macOS arm64（最低 macOS 15.0）。macOS 已进行本机编译检查，并补充 Retina 截图坐标回归测试。多屏交互、系统授权、签名公证及升级安装仍需实际验收，自动检查不代表这些交互已经通过。

代码迁移没有发布新版本或切换更新源。原生候选可在本地构建，或运行默认 Windows 的 `native-candidate` 工作流；发布工作流只准备带签名的草稿，公开发布和更新源切换仍是后续操作。

## 开发

Windows 需要 `rust-toolchain.toml` 指定的 Rust、MSVC C++ Build Tools 和 Windows SDK；打包需要 NSIS 3.11。原生构建不需要 Node.js、Yarn、浏览器运行时或前端构建命令。

macOS 需要 Apple Silicon Mac、macOS 15.0 或更新版本以及 Xcode Command Line Tools（`xcode-select --install`）。使用相同的 Cargo 命令；`.app` 和 `.dmg` 构建见[打包说明](../native/README.md)。截图需要系统“屏幕与系统音频录制”权限，划词翻译需要“辅助功能”权限。请给实际运行的应用授权，终端开发运行与打包后的应用可能需要分别授权。

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

应用及共享 crate 均位于 `crates/`，模型、字体与图标位于 `assets/`；`xtask/` 管理版本、暂存、打包与签名，`native/` 保存分发配方。

开发身份默认使用 `.rotor-gpui`，并在保存的全局快捷键上增加 Alt。正式身份使用 `.rotor`，通过 `production` feature 启用。可用 `--data-dir` 或 `ROTOR_DATA_DIR` 指定资料目录。

| 操作 | 保存的 Windows 快捷键 | 开发模式实际快捷键 |
|---|---|---|
| 文件搜索 | Ctrl+Shift+F | Ctrl+Alt+Shift+F |
| 截图 | Ctrl+Shift+S | Ctrl+Alt+Shift+S |
| 划词翻译 | Ctrl+Shift+D | Ctrl+Alt+Shift+D |
| 输入翻译 | Ctrl+Shift+W | Ctrl+Alt+Shift+W |

开发模式设置入口为 Ctrl+Alt+Shift+G。贴图局部默认键为 S 保存、Enter 复制、H 隐藏、Escape 关闭；文字编辑时采用独立的确认/取消行为。

macOS 保存的全局快捷键使用 Cmd+Shift+F/S/D/W；开发模式额外增加 Option。划词复制使用 Cmd+C。

## 原生打包

```powershell
cargo run -p xtask -- build
cargo run -p xtask -- stage target/native-stage
$env:NSIS_MAKENSIS = 'C:/Program Files (x86)/NSIS/makensis.exe'
cargo run -p xtask -- package target/native-stage target/native-package
```

暂存和包目录必须是新目录。正式身份需同时给 build 和 stage 传入 `--production`。签名、资料备份、回退及静默安装检查见[分发说明](../native/README.md)。

版本以根 Cargo workspace 为准：

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- set-version 2.7.0-beta.1 --dry-run
```

版本工具不会自动提交、打标签或推送。

## 贡献与许可证

共享业务 crate 不引入 GPUI、Tauri 或 WebView 依赖。测试使用独立合成资料，失败与回退时保留用户数据。

项目采用 [MIT License](../LICENSE)。文字标注使用系统已安装的字体。
