# GPUI 迁移候选验收记录（2026-09-08）

当前状态：**代码与 Windows 候选已准备；迁移没有完成，G8/G9 未通过。**

构建源码提交：`76f0352ddd0af2ef3765723151340710a132c976`。文档收尾提交不改变原生输入摘要。
共同源码/资源/安装配方摘要：`be09038648c8fa0b7f4d3183d2dfaa4cfc3a15881fca3c81461e34e0105f4b88`。

## Windows 候选

版本仍为 2.6.0，用于本地验收准备。产物未使用真实更新私钥签名，未安装、未上传、未发布；不应据此切换旧 latest 通道。

| 模式 | 产物（仓库相对路径） | 字节 | SHA-256 |
|---|---|---:|---|
| development | `target/native-final/development-package/Rotor-GPUI_2.6.0_x64-setup.exe` | 37,531,440 | `a0eadbc4b1eda62247d1619ee234467e6f4b78da587234d630c010a294bfbccb` |
| production | `target/native-final/production-package/Rotor_2.6.0_x64-setup.exe` | 37,567,235 | `699a233710277aaa2b614c9f3ff9edfe5bf2c13c6f00d8847360691c3b1611c6` |

两种模式各自的 stage 目录均有 31 个已核对文件，package 目录均有 3 个已核对文件；文件集合、大小及 SHA-256 全部匹配 resources.json。源码收据一致。

## 已有验证证据

- 最终 native/core 自动测试：125 项通过；包含配置、画布、运行时、搜索、截图、翻译、UI 状态、更新器及 xtask。
- 最终原生相关 clippy 与指定 crate 格式检查通过。
- 最终 cargo check --workspace 通过；旧 Tauri 留有 3 项既有 warning。
- 旧 yarn build 通过，旧壳继续可构建。
- Windows x64 / macOS arm64 核心依赖图检查通过；这不代表 macOS 编译通过。
- 两种模式此前的 --build-info / --check-resources 诊断通过；正式暂存资源的合成中文/英文 OCR 与闲置释放通过。
- 旧 v2.6.0 Windows 安装包及 macOS 更新归档均通过现有公钥验签。
- Windows 安装包内部身份/版本只读核验通过，错误模式及伪高版本被拒绝。
- 合成数据副本的旧格式 → native 写回 → legacy Rust 读取器及二次导入通过；没有用正式安装版读写真实用户数据。
- 最终两个候选的文件清单和源码摘要已只读核验。最终再次批量启动候选做诊断的命令被自动审批审查拒绝（blocked by policy），未执行，也未绕过。

详细命令输出保留在被 Git 忽略的 experiments/gpui-probe/artifacts/；摘要证据位于 evidence/windows-candidates-2026-09-08.json。

## 仍需完成的门槛

1. **Windows 原生交互**：此前桌面控制因 Esc 停止；本轮不再操作。需要新一轮验证 IME、焦点、混合 DPI/负坐标、多屏截图、画布与 OCR、快捷键录制和连续保存操作。
2. **macOS arm64**：尚无本轮可用 Mac 编译/运行结果；需要已有 CI 或用户提供的 Mac，核对窗口、权限、剪贴板、LaunchAgent、更新交接及 app/DMG。
3. **安装与更新**：真实 UAC、首次安装、旧客户端接收、覆盖升级、卸载、启动失败/文件失败恢复、断网和坏签名场景尚未执行。Windows 自动回退当前针对安装完成后普通启动错误路径；装载错误、崩溃和挂起必须独立验证，不能据编译通过宣称全部恢复情形已覆盖。
4. **候选签名与版本**：选择实际候选版本，使用现有更新密钥在审查后的候选工作流中签名；没有自动提交 tag、push 或发布。旧 macOS 系统与旧客户端的更新分流必须先演练。
5. **性能与最终切换**：固定机器常驻曲线、多贴图/重复 OCR/流式翻译压力与全部功能验收通过后，才执行 P9 的旧壳删除、目录搬迁与正式文档切换。

## 保留与恢复原则

默认构建仍使用独立开发身份。production feature 只用于明确的正式身份验收；首次真实启动会先保护旧实例命名空间并备份资料。安装/回退保留旧程序、失败程序与数据备份；二进制恢复不自动覆盖迁移后的截图。

保留旧壳是迁移计划的验收前置条件，不能为了将进度标为完成而提前删除。
