<p align="center"><img width="100" src="./branding/logo.png" alt="Rotor logo"></p>

# Rotor

使用 Rust、GPUI 和 gpui-component 构建的原生桌面工具箱。

[English](../README.md) · [迁移状态](gpui-migration/remaining-tasks.md) · [打包与恢复](../native/README.md)

## 功能

- 文件索引搜索、键盘导航、目录排除及 Windows 管理员打开。
- 多屏截图、贴图、裁剪缩放、画笔/矩形/箭头/文字标注、PNG 与剪贴板导出。
- 使用内置 ONNX 模型和字体的本地中英文 OCR。
- 输入及划词翻译，支持 Google、DeepSeek 和自定义 HTTP 引擎。
- 快捷操作、快捷键录制、设置自动保存、中英文及浅色/深色主题。
- 原生托盘、单实例、自启动与更新签名校验。

## 当前平台状态

当前验收范围为 Windows x64，本机检查使用 Windows 11。macOS arm64 实现与打包配方仍保留，配置最低版本为 macOS 15.0，但 macOS 验收暂缓。后续可视化测试已按用户要求跳过；已通过、跳过和未完成项目见迁移记录。

代码迁移没有发布新版本或切换更新源。原生候选可在本地构建，或运行默认 Windows 的 `native-candidate` 工作流；发布工作流只准备带签名的草稿，公开发布和更新源切换仍是后续操作。

## 开发

Windows 需要 `rust-toolchain.toml` 指定的 Rust、MSVC C++ Build Tools 和 Windows SDK；打包需要 NSIS 3.11。原生构建不需要 Node.js、Yarn、浏览器运行时或前端构建命令。

```powershell
cargo run -p rotor-desktop -- --no-elevate --data-dir target/dev-profile
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

应用及共享 crate 均位于 `crates/`，模型、字体与图标位于 `assets/`；`xtask/` 管理版本、暂存、打包与签名，`native/` 保存分发配方。独立 P0 实验保留在 `experiments/gpui-probe`。

开发身份默认使用 `.rotor-gpui`，并在保存的全局快捷键上增加 Alt。正式身份使用 `.rotor`，通过 `production` feature 启用。可用 `--data-dir` 或 `ROTOR_DATA_DIR` 指定资料目录。

| 操作 | 保存的 Windows 快捷键 | 开发模式实际快捷键 |
|---|---|---|
| 文件搜索 | Ctrl+Shift+F | Ctrl+Alt+Shift+F |
| 截图 | Ctrl+Shift+S | Ctrl+Alt+Shift+S |
| 划词翻译 | Ctrl+Shift+D | Ctrl+Alt+Shift+D |
| 输入翻译 | Ctrl+Shift+W | Ctrl+Alt+Shift+W |

开发模式设置入口为 Ctrl+Alt+Shift+G。贴图局部默认键为 S 保存、Enter 复制、H 隐藏、Escape 关闭；文字编辑时采用独立的确认/取消行为。

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

版本工具不会自动提交、打标签或推送。旧源码清理单独跟踪，它们已不再参与原生 workspace 和 CI 构建。

## 贡献与许可证

共享业务 crate 不引入 GPUI、Tauri 或 WebView 依赖。测试使用独立合成资料，失败与回退时保留用户数据。

项目采用 [MIT License](../LICENSE)；内置 Noto Sans CJK 的 OFL 许可证位于 `assets/fonts/`。
