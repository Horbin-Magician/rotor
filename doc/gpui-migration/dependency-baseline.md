# P0 依赖与平台基线

日期：2026-09-07。原型位于 `experiments/gpui-probe`，独立 workspace，不加载业务 crate。**Windows 原型已构建并完成一轮实机验证，G0 尚未通过**；macOS 和完整旧版基线仍有未测项。

## 锁定组合

| 项目 | 版本 / features |
|---|---|
| Rust/Cargo | 1.97.0；原型 rust-toolchain.toml 独立固定 |
| gpui-kit | =0.6.0；default-features=false，features=component,assets |
| gpui-component/base/assets | 均为 0.6.0，见 Cargo.lock |
| gpui-pre / gpui-pre-platform | =0.3.3；整个配套平台系列为 0.3.3 |
| 系统事件 | global-hotkey =0.7.0、tray-icon =0.21.3、async-channel =2.5.0 |
| 像素与离屏 | image =0.25.10（直接启用 png）、resvg =0.45.1 |
| Windows 原生适配 | raw-window-handle =0.6.2、windows =0.58.0（Foundation/Gdi/WindowsAndMessaging） |
| 错误 | anyhow =1.0.102 |

Kit 0.6.0 发布包 `.cargo_vcs_info.json` 给出的仓库 SHA：`94a313a72a2513aee2780240cd322d552b2395f0`，路径 crates/kit。[Kit 来源](https://github.com/longbridge/gpui-kit/tree/94a313a72a2513aee2780240cd322d552b2395f0)。

最终候选 `gpui-pre` 0.3.3 的发布 manifest metadata 标注 `zed-crate=gpui`、`zed-version=0.2.2`、`zed-rev=5b055fa789a8b8d38ac951a6e0cde272f66b4495`。这是发布者声明的来源关系，不是零补丁证明；不能混用上游 gpui 0.2.2 文档。[配套 API](https://docs.rs/gpui-pre/0.3.3/gpui/)。

没有修改 registry 中的源码或引入 vendor 补丁。Windows 遮罩的客户区校正在原型 `src/platform.rs`：通过借用的 HWND 获取实际显示器物理边界，补偿非客户区，最多校正两次并验证结果。后续 P2 吸收这段边界代码，需保留多 DPI/负坐标/热插拔回归；不能将本次单机成功扩展为所有显示器拓扑已通过。

## 候选淘汰记录

首轮 0.3.1（上游标注 `801c087af22dd189dc1aa49e2f370b4f04190b19`）在 Windows 的 check 与 clippy 通过，但 release 在 `gpui-pre-macros/src/gpui_macros.rs:377` 失败，E0433。`derive_inspector_reflection` 模块受 inspector/debug_assertions 条件保护，其包装函数却没有同样的 cfg。

已核对 0.3.3 发布源码，辅助函数增加了对应 cfg。改用整个 0.3.3 配套系列后 release 和测试通过；没有通过启用 inspector 或保留 debug assertions 绕开问题。原错误摘要和本轮结果见 [Windows 证据](evidence/p0-windows.md)。

## 系统边界

- [官方安装文档](https://gpui-kit.com/docs/installation/)列出 Windows 10+、macOS 15+、Rust 1.90+，属于候选开发要求，不作为已实测的最低运行系统。
- 本机：Windows 11 Pro 10.0.26200，x86_64；Intel Core Ultra 7 265K（20 逻辑核），Intel Graphics 32.0.101.7029，另有 GameViewer Virtual Display Adapter 15.6.5.199。两屏均 200% DPI，物理边界 `(0,0) 3840×2160`、`(3840,-843) 2160×3840`。
- Windows release 使用本机 MSVC/Windows SDK；SDK 自带 fxc 可用（最高已安装 10.0.26100.0），NSIS 3.11 取自本机 Tauri 工具缓存。
- macOS arm64 设备不可用。已增加 macos-15 CI 配方和 app 资源布局，但未运行 CI/本机 macOS，不能算构建或运行通过。最低 OS 需以 Mach-O load commands 和真实系统启动结果共同冻结。

## 实现与剩余门槛

| 任务 | 本轮交付 / Windows 证据 | 尚需验证 |
|---|---|---|
| P0-01 | 源码基线、已安装 v2.6.0 身份、脱敏配置子集、合成贴图记录、环境/进程树采样脚本及初步待机 CSV | 旧版功能截图/录屏、真实贴图副本、zoom×DPI 导出矩阵和完整性能预算 |
| P0-02 | 精确依赖、锁文件、工具链；Windows check/clippy/release/test 通过，两端依赖闭包通过 | macOS release、本机窗口、最低系统 |
| P0-03 | 普通窗口+两透明贴图+每屏遮罩；双屏客户区物理边界校验；全窗口关闭后常驻及热键恢复通过 | 混合 DPI、热插拔、更多焦点/置顶情形、macOS Spaces/fullscreen |
| P0-04 | 中文 IME 组合/提交/取消；PNG 系统剪贴板逐像素回读；原生保存确认/取消与焦点返回；托盘/热键同事件循环初始化 | 托盘菜单实点、外部应用粘贴、保存失败注入、macOS 对应场景 |
| P0-05 | RGBA→BGRA、共享纹理、物理像素 1:1、PNG 与中文/线条两项 release 测试通过 | 复杂文字/字体回退、GPU 不可用注入、更多色彩/清晰度设备 |
| P0-06 | 实际生成独立 Windows NSIS 包；macOS app/plist/Resources 配方；更新交接草案与真实 v2.6.0 元数据 | 安装/卸载运行、macOS 实际包、旧产物本体/验签与升级演练 |

P0 的固定 640×240 PNG 不是正式 P6 导出语义。原型不读取 `.rotor`、不执行更新；macOS Dock 策略仅由 app 的 LSUIElement 提供。未自动响应显示器热插拔。

## 可复现命令

```text
cargo +1.97.0 fmt --manifest-path experiments/gpui-probe/Cargo.toml --all -- --check
cargo +1.97.0 check --manifest-path experiments/gpui-probe/Cargo.toml --locked
cargo +1.97.0 clippy --manifest-path experiments/gpui-probe/Cargo.toml --all-targets --locked -- -D warnings
cargo +1.97.0 test --manifest-path experiments/gpui-probe/Cargo.toml --release --locked
cargo +1.97.0 build --manifest-path experiments/gpui-probe/Cargo.toml --release --locked
```

运行、采样、依赖审计与打包见 [原型 README](../../experiments/gpui-probe/README.md)。本轮网络受限，构建使用临时官方索引转发及已校验缓存，并附 `--offline --config ...`；没有提交临时 source 配置，锁文件仍使用 crates.io 标准来源和 checksum。

## 本轮结果

见 [Windows 证据](evidence/p0-windows.md)、[旧版基线](baseline.md)、[更新交接](update-handoff.md)。只检查目标平台 normal/build 闭包；锁文件中的 wasm 或其他平台依赖不等于本机运行时依赖。最终 Windows/macOS 闭包不含 Tauri 运行时、Wry/WebView、GPUI Shell、QuickJS。
