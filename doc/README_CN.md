<p align="center"><img width="100" src="../assets/icons/icon.png" alt="Rotor logo"></p>

# Rotor

使用 Rust、GPUI 和 gpui-component 构建的原生桌面工具箱。

[English](../README.md) · [验收状态](validation-status.md) · [打包与恢复](../native/README.md)

## 功能

- 文件索引搜索、键盘导航、目录排除及 Windows 管理员打开。
- 多屏截图、贴图、裁剪缩放、画笔/矩形/箭头/文字标注、PNG 与剪贴板导出。
- 使用内置 ONNX 模型的本地中英文 OCR；文字标注使用系统字体。
- 输入及划词翻译，支持 Google、DeepSeek 和自定义 HTTP 引擎。
- 快捷操作、快捷键录制、设置自动保存、中英文及浅色/深色主题。
- 原生托盘、单实例、自启动与更新签名校验。

## 当前平台状态

发布草稿默认准备 Windows x64 与 macOS arm64（最低 macOS 15.0）双平台。
已执行的检查、安装结果及待完成的人工验收分别记录在[验收状态](validation-status.md)。
CI 矩阵中的平台配置不能作为该平台实际验收通过的依据。

Rotor 3 使用全新的资料与安装命名空间，不提供 2.x 导入或覆盖升级，原有用户资料保持不变。
公开发布、镜像同步及 stable/preview 推广见[发布操作说明](../native/release-operations.md)。

使用合成资料拍摄的[原生界面截图](screenshots/README.md)展示了当前搜索、截图标注、翻译和设置界面。

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

应用及共享 crate 均位于 `crates/`，模型与图标位于 `assets/`；`xtask/` 管理版本、暂存、打包与签名，`native/` 保存分发配方。

开发身份默认使用 `.rotor3-dev`，并在保存的全局快捷键上增加 Alt。正式身份使用 `.rotor3`，通过 `production` feature 启用。可用 `--data-dir` 或 `ROTOR_DATA_DIR` 指定资料目录。

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

暂存和包目录必须是新目录。正式身份需同时给 build 和 stage 传入 `--production`。签名、原生更新恢复及静默安装检查见[分发说明](../native/README.md)。

版本以根 Cargo workspace 为准：

```powershell
cargo run -p xtask -- version
cargo run -p xtask -- set-version 3.0.0 --dry-run
```

版本工具不会自动提交、打标签或推送。

## 贡献与许可证

共享业务 crate 不引入 GPUI、Tauri 或 WebView 依赖。测试使用独立合成资料，失败与回退时保留用户数据。

项目采用 [MIT License](../LICENSE)。文字标注使用系统已安装的字体。
