# Windows 静默安装与失败保留（2026-09-08）

按用户要求跳过可视化测试，继续非可视化安装/恢复验证。执行前确认当前 PowerShell 已提权，开发身份的 HKLM 注册项、开始菜单快捷方式和用户自启动值均不存在；脚本遇到已有开发安装会拒绝执行。全部安装/备份/合成资料使用仓库 `target/` 下的新目录，没有替换正式 Rotor 安装或访问正式资料。

## 实际发现并修复的问题

最初静默首装返回 2，没有生成正确安装目录。加入可选 `/LOG=<path>` 后观察到规范化后的安装路径为空。NSIS 的 [GetFullPathName](https://nsis.sourceforge.io/Reference/GetFullPathName) 在路径不存在时可能清空输出，原实现直接覆盖了新安装目录变量。现改用不要求目标已存在的 [Windows GetFullPathNameW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew)，检查失败/过长返回值后才采用规范化路径。

全部错误/通知使用统一消息宏，[`/SD IDOK`](https://nsis.sourceforge.io/Reference/MessageBox) 让 `/S` 路径不再等待对话框，原来的 Abort/失败语义保留。可选诊断日志为带 BOM 的 UTF-16LE，中文安装路径在日志中完整保留；默认不创建日志。

## 验证结果

修复后的最终包在 `target/windows-silent-install-run4/安装 Rotor` 实际执行，结果见[机器可读记录](windows-silent-install-2026-09-08.json)：

- 非空无关目录被拒绝，退出 2，合成原文件及注册状态不变。
- 全新中文/空格目录安装成功，退出 0；开始菜单/安装注册项正确，安装文件逐一通过 resources.json 大小/哈希核对。
- 安装目录内的实际程序通过 `--build-info` 和 `--check-resources`，资源指向安装目录，不依赖开发环境覆盖或当前工作目录。
- 持有旧程序的禁止删除共享句柄时，覆盖安装返回 2，旧程序哈希保持不变；解除占用后同版本替换成功，旧安装及用户新增文件保留在备份目录。
- 静默卸载退出 0，程序、所有包内文件、开发身份 HKLM 项和开始菜单快捷方式移除；安装目录仅留下测试新增文件，独立合成配置哈希不变，升级备份仍保留。

测试脚本为 [.github/scripts/test-windows-install.ps1](../../../.github/scripts/test-windows-install.ps1)，限制开发身份、新工作区 target 子目录和空闲注册命名空间。测试资料/失败阶段目录/备份保留，未递归清理未知内容。

候选 workflow 新增 Windows/both/macOS 选择，默认 Windows；开发身份 Windows 包默认运行静默安装检查并上传小型日志/结果文件。完整签名元数据仍要求 both。PowerShell 语法和 workflow YAML 本地解析通过，未运行远端工作流、签名或发布。

此结果不证明递增版本升级、旧 Tauri 客户端交接、正式身份安装、自启动登录清理或崩溃/挂起回退。交互式 UAC 和视觉/桌面矩阵按用户要求跳过，macOS 暂缓；G8 的其余非可视化条件继续保留。
