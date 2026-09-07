# P0 Windows 实机证据

日期：2026-09-07。执行：Codex，本地 Windows UI 自动化及命令行。应用源码状态：`b084f4d` + 本次未提交的 P0 变更；旧源码基线为 `40addee065765ed343e665d6a712b3e9cbe2bdcc`。

## 环境与产物

- Windows 11 Pro 10.0.26200 / x86_64，Core Ultra 7 265K（20 逻辑核）。Intel Graphics 32.0.101.7029；存在 GameViewer Virtual Display Adapter 15.6.5.199。
- Rust/Cargo 1.97.0，GPUI 0.3.3 + Kit/Component 0.6.0，release 优化构建，NSIS 3.11。
- 两屏 DPI=2；主屏物理 `(0,0) 3840×2160`，竖屏 `(3840,-843) 2160×3840`。
- Cargo.lock SHA256：`A0FA2C51BA7BB5C8F6015CE92A3261C405A948F84082E0875D6D520B06340BBA`。
- 原型 exe SHA256：`35BF8F0D2CB0078EE0653027078BD6D225754FEF2EA4D1801CC163A5B629AA9E`。
- NSIS SHA256：`8CF6AAF7924481EEF847B421871DD9E4EDA0DA73BAE2B21CF3A706BF4D5F704E`。独立身份，未签名、未安装、未发布。
- 离屏 PNG 和原生保存框导出 PNG 的 SHA256 相同：`FCB86244BAD672F03D76442EDD4B61FE041A26FE4C965D2CFE8EDD9CA5A60E52`。

本地详细日志/样例在 `experiments/gpui-probe/artifacts`（git 忽略）；此文件保留可提交的关键结果，未提交含其他应用背景内容的桌面截图。

## 结果

| 验收项 | 操作与实际结果 | 状态 |
|---|---|---|
| 构建 | 最终 release build 成功；fmt、clippy all-targets -D warnings 通过 | 通过 |
| 像素测试 | 两项 release 单测：通道顺序/alpha/PNG 无损回读；中文区域/线条/输出尺寸；2 passed, 0 ignored | 通过 |
| 依赖 | Windows x64 与 macOS arm64 normal/build 目标闭包均无禁止的 UI/JS 运行时 | 通过（不是 macOS 构建） |
| V01/V02 窗口 | 默认同时创建普通窗、两张透明贴图、每屏遮罩；200% DPI 下实机可见 | 早期通过；未覆盖全部 V01/V02 |
| V02 客户区边界 | 初版主屏遮罩 y=-3 逻辑像素、捕获高度 1078；Windows 边界适配后 y=0、高度 1080，原生断言覆盖两屏物理矩形 | 本机通过 |
| V04 IME | 输入法切到中文，n→i 显示候选“你”，空格提交；再次 n 后 Escape 取消，保留已提交文本及窗口 | 通过此组合/提交/取消场景 |
| V05 原生保存 | 保存到 `artifacts/原生 保存.png`，窗口显示成功状态且可继续操作；哈希与 CLI 样例相同 | 通过 |
| V05 保存取消 | 再次打开保存框，Escape 取消，显示“已取消；窗口保留”，输入未丢失 | 通过 |
| V05 PNG 剪贴板 | Copy 后从系统剪贴板回读图像，解码后所有 RGBA 像素与源图一致 | 回读通过；外部粘贴未测 |
| V09 清晰度 | 修正初版按逻辑像素放大的模糊；图像按 source pixels / DPI 显示，原生文字按同物理字号对照 | 本机早期通过；非逐字形一致性认证 |
| 常驻/热键 | `--window-only` 下关闭唯一窗口，可见窗口列表为空、进程 MainWindowHandle=0 仍存活；从仓库编辑器按 Ctrl+Alt+Shift+G 后生成新的原型窗口 | 通过 |
| 退出 | UI 退出关闭全部原型窗口，前台命令返回 exit code 0 | 通过 |
| 托盘 | 同一主线程成功创建托盘和菜单，并保留资源到退出 | 初始化通过；菜单实点未测 |

遮罩最终运行日志：

```text
mask physical client: (0, 0) 3840x2160
mask physical client: (3840, -843) 2160x3840
```

## 本轮发现并修复

1. GPUI 0.3.1 的 release 宏 E0433：模块受 cfg 保护，包装函数未受保护。改用已修复的完整 0.3.3 系列；未启用 inspector。
2. 深色父视图与浅色输入控件主题冲突导致文字不可见：显式采用组件库 Dark 主题，复测英文和中文候选提交可见。
3. 200% DPI 下测试位图被放大：按物理像素显示并对齐原生文字字号。
4. 遮罩客户区偏移：Windows 适配层补偿非客户区并校验实际 monitor rect。

## 未通过或未测的边界

- 没有 macOS 本机或已执行的 macOS CI 结果；最低系统版本未冻结。
- 未验证混合 DPI、热插拔、全屏/Spaces、GPU 失败注入、复杂字体回退。
- 系统画图无法启动，故不宣称外部应用粘贴兼容通过；剪贴板回读是独立、较窄的证据。
- NSIS 生成通过，不等于安装、卸载或旧客户端升级通过。没有向正式更新通道发布。
- 旧版完整性能曲线、旧 Konva 导出矩阵及真实贴图样本仍需补齐；G0 保持未通过。
