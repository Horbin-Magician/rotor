# 当前验收状态

- Windows x64 是当前工程检查目标；正式发布和更新源推广尚未完成。
- 后续可视化及人工 UI 验收按用户要求跳过，不计为通过。
- 2026-09-09 移除首次启动时自动打开设置窗口的逻辑，默认后台常驻；托盘、设置快捷键及再次启动激活已有实例仍可打开设置。`cargo fmt --all -- --check`、`cargo check -p rotor-desktop --locked` 通过；视觉 UI 验证跳过，macOS 验证暂缓。
- Windows 托盘右键菜单使用 Win32 原生菜单及 GDI 自绘，仅保留设置和退出；复用托盘窗口并缓存字体和画刷，支持应用主题、系统主题及高对比度。主题切换、混合 DPI 定位、读屏及鼠标/键盘交互的人工验收跳过；实际呼出延迟尚未测量。
- 本次原生托盘菜单修改的非视觉检查通过：`cargo fmt --all -- --check`、`cargo check -p rotor-desktop --locked`、`cargo clippy -p rotor-desktop -p rotor-ui --all-targets --locked -- -D warnings`、`cargo test -p rotor-desktop -p rotor-ui --locked`（主程序 5 项、恢复程序 2 项、UI 29 项）。新增测试覆盖隐藏的合成原生窗口消息、菜单文字/ID 保留、资源缓存及离屏 GDI 绘制，不计为视觉验收。
- macOS 构建、运行、安装、签名和性能验收暂缓；Windows 结果不替代 macOS 验收。
- 托盘菜单样式采用约 6 DIP 圆角、低对比度边框、文字居中，基准尺寸 80×28 DIP，取消空勾选栏并保留字体测量和 DPI 缩放。用户反馈上一轮 `WM_PAINT` 后补画仍未解决实际亮边问题，不能视为修复验收通过。
- 本轮补齐此前遗漏的路径：在 `WM_INITMENUPOPUP` 安装短期线程钩子，按 HMENU 匹配窗口并在定位/显示前接管；关闭 DWM 非客户区绘制及 Windows 11 系统边框；`WM_PRINT` 使用调用者提供的 DC 绘制圆角边框；尺寸变化时更新裁剪区域，并避免 `SetWindowRgn` 重入循环。`cargo fmt --all -- --check`、`cargo check -p rotor-desktop --locked`、`cargo clippy -p rotor-desktop -p rotor-ui --all-targets --locked -- -D warnings`、两包 `tray_menu::tests`（桌面 2 项、UI 3 项）及 `cargo build -p rotor-desktop --locked` 通过，已生成开发版可执行文件。新增隐藏合成窗口检查覆盖真实线程钩子、进入空闲前接管、重复 `WM_PRINT` 的目标 DC、隐藏可见性标志、裁剪区域及钩子清理；既有离屏检查覆盖浅/深主题及 96/144/192 DPI。实际弹出菜单视觉验收按要求跳过，当前显示问题的现场解决情况仍未确认；这些非视觉检查不等同于实际原生弹出菜单验收。

- 针对用户反馈的圆角锯齿，最新实现优先启用 Windows 11 DWM 小圆角及指定主题边框色，清除窗口裁剪区域，并停止叠加 GDI 圆弧描边，由系统负责外缘合成；不支持这些接口的系统继续使用原 GDI 圆角兼容路径，兼容路径不具备外缘抗锯齿。当前尺寸和文字样式保持不变。最新 `cargo fmt --all -- --check`、两包 Clippy（全部 targets、拒绝警告）、两包 `tray_menu::tests`（2+3 项）及开发版构建通过。隐藏窗口检查验证已接受的圆角偏好和裁剪区域移除；离屏检查验证合成路径不再绘制 GDI 圆弧、保留内容及重复绘制。DWM 接口成功仅表示接受偏好，不代表屏幕圆角效果已验收；实际视觉验收按要求跳过。

- 针对最新截图中的顶部白线和呼出抖动，本轮补充 `WM_NCACTIVATE` 处理：保留激活逻辑，以 `lParam=-1` 禁止系统激活描边；DWM 按菜单窗口样式处理外框并同步应用深浅主题，关闭该窗口的 DWM 过渡。接管锁定版本 tray-icon 0.21.3 的托盘菜单点击通知，在原托盘 owner 上用 `TPM_NOANIMATION` 调用原 HMENU，保留 muda 的命令派发，并在结束后发送 `WM_NULL`。不修改全局动画设置。无窗口区域时不调用 `SetWindowRgn`，初始化期间抑制重入的兼容裁剪，减少重复定位/重绘。本轮格式检查、两包 Clippy、托盘测试（2+3 项）通过；隐藏窗口检查新增激活消息不重绘及无区域时清理不产生定位消息的断言。实际顶部白线和呼出稳定性仍未完成视觉验收，按要求记录为跳过。

- 本轮 `cargo build -p rotor-desktop --locked` 已完成链接，但因运行中的 `target/debug/rotor-desktop.exe` 被占用，在替换最终文件时失败，不计为构建命令通过。已将本轮链接产物复制为 `target/debug/rotor-desktop-tray-fix-20260909-131002.exe`，校验副本哈希一致，`--build-info` 与 `--check-resources` 通过；未启动窗口或替换当前进程。

## 尚未完成

- 在隔离 Windows 环境完成正式身份首装、递增升级、登录自启动和卸载验收。
- 完成旧客户端与原生客户端的真实更新链、签名、下载故障、UAC 取消及安装恢复矩阵。
- 补齐启动崩溃、挂起、恢复自身失败的完整安装链测试。
- 确认最低支持系统、升级兼容范围及 macOS 更新路由。
- 完成固定语料下的性能预算验证：冷启动、交互延迟、待机 CPU、长时间常驻和资源回落。
- 截图路径已实施 Windows 窗口/缓冲区预热复用、独立有界 worker、BGRA 直通和分段日志；快捷键到实际呈现 <50ms 尚未验收。测量阶段及跳过项见 [截图性能说明](capture-performance.md)。
- 配置本机 GitHub 登录及签名环境，执行远端候选/发布工作流，审查草稿后推广更新源。

工程检查命令见 [开发说明](../README.md)，打包及恢复步骤见 [分发说明](../native/README.md)。
