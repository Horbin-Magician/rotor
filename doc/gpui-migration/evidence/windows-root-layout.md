# Windows 根目录工程迁移（2026-09-08）

用户明确要求跳过所有可视化测试并继续迁移。当前隔离桌面程序和本地翻译服务已按已核对的进程路径/参数停止，原合成数据保留；不再等待人工 `short` 输入。可视化项按用户要求跳过，macOS 验收继续暂缓。

## 目录与依赖

- 六个共享 crate 从 `src-tauri/crates/` 移至根 `crates/`，与 desktop/UI/canvas/updater 并列。根 workspace、旧壳依赖及 runtime→canvas 相对路径已同步。
- 模型、字体、图标从 `src-tauri/assets/` 移至根 `assets/`。移动前后 25 个文件 SHA-256 逐一相同；目录没有符号链接/reparse point。原生编译嵌入图标、字体测试、OCR 示例、xtask 和字体检查脚本改用新路径。
- 旧 Tauri 适配继续位于 `src-tauri/src/integration`，通过 `{"../assets/": "assets/"}` 映射保留安装资源路径；图标源改为 `../assets/icons/...`。此映射使用锁定的 tauri-utils 目录资源映射，保留子目录结构。
- 配方继续使用 `native/`，无需再引入同义目录。构建收据改为覆盖根 `crates/`/`assets/`，并纳入 xtask、`.cargo` 与 `.gitattributes`，避免构建/资源规则变更后沿用旧快照。
- AGENTS、开发命令、原生打包说明和功能表实际源码链接同步更新；历史证据中的旧命令/哈希不改写。

## 验证

`cargo check --workspace --offline` 通过，包括旧 Tauri 消费方；旧壳保留三项既有 unused/dead_code warning。

`cargo test --workspace --offline` 全部 174 项通过：旧适配 2、canvas 12、common 14、desktop 4、platform 14、runtime 34、screenshot 16、searcher 18、translator 15、UI 25、updater 13、xtask 7。示例只编译，未执行窗口入口。命令日志保留于忽略目录 `target/layout-workspace-tests.log`。

严格 clippy 首次发现 platform 的测试模块之后仍有函数；仅移动测试模块至文件末尾后，`cargo clippy --workspace --exclude rotor --all-targets --offline -- -D warnings` 通过。未增加 lint 抑制，旧壳 warning 没有冒充严格检查通过。

字体及许可证哈希检查通过。Windows 共享核心依赖边界检查通过；native normal/build 依赖闭包不含 Tauri、Wry、WebView2 或 QuickJS。格式和 diff 检查通过。下一阶段从这份目录结构重新构建和打包，旧目录布局的候选不作为新布局的产物证明。

## 新布局产物与无窗口集成

基于目录迁移提交 `4f1f107`，xtask 离线 release、暂存及 NSIS 3.11 开发安装包构建均通过。产物为 `target/native-root-layout-4f1f107/package/Rotor-GPUI_2.6.0_x64-setup.exe`，37,609,281 字节，SHA-256 `e0a71e3266b5abc6fa5d427e7f6ad6a387f5d0f4517ce0d4a923a2f0fe1fff90`；[完整记录](windows-root-layout-candidate-2026-09-08.json)包含源码收据、程序哈希与诊断输出。

31 个暂存文件与 3 个包目录文件通过清单校验，安装器产品名/版本经实际 Windows VERSIONINFO 读取器核对。清除子进程资源/数据覆盖，在 `D:\` 执行中文/空格暂存路径的 `--build-info`、`--check-resources` 均通过。debug 默认资源探测也通过，实际选中相邻的 `target/debug/assets`，没有把它误报为根 assets fallback 的运行证据。

使用暂存模型及字体完成无窗口 OCR：识别出“中文测试 Hello 123”和“Rotor OCR验证”，等待现有 idle reaper 后确认模型缓存释放。新的 `target/profile-root-layout-4f1f107` 合成资料完成旧格式 → native 写回 → legacy Rust 读取器及二次导入；生产身份 common 的 14 项测试通过。未运行安装器、未签名或发布；可视化测试按用户要求跳过，macOS 暂缓。P9-02 的 Windows 工程目录迁移收口，余下安装恢复及最终清理继续单独推进。
