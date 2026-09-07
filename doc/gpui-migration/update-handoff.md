# P0 更新交接草案

## 旧身份与渠道（源码核对）

- 产品 Rotor，identifier `cc.fluctus.rotor`，版本来自 package.json（v2.6.0 基线）。
- Windows：NSIS、perMachine、SimpChinese/English、安装器带 WebView bootstrapper；未配置证书指纹。不能据此推断实际历史安装目录、卸载键或 Authenticode 状态，必须提取旧包安装结果。
- macOS：app/dmg，未显式配置最低系统版本和 signingIdentity。不能把缺省值当作已验证的最低运行版本。
- `bundle.createUpdaterArtifacts=true`；公钥和 Gitee/GitHub latest.json 地址在 `src-tauri/tauri.conf.json`。发布工作流创建 draft；同步工作流会改写元数据中的 GitHub URL。
- `.rotor` 数据身份、旧自启动项、Windows 提权行为必须单独验证；新 GUI 名称不能替代这些兼容要求。

## P0 试验包

`experiments/gpui-probe/packaging/windows.nsi` 使用独立 Program Files 目录和 `RotorGpuiP0` 卸载键；macOS 使用 `cc.fluctus.rotor.gpui-probe`、`LSUIElement=true`。试验包没有更新客户端，不下载或发布更新。Windows 卸载只移除试验可执行文件及其卸载程序，不删除 `.rotor` 或共享 WebView2。

资源：Kit 图标内嵌，中文字体使用系统字体；Resources 中放置试验说明。OCR 不在原型中。macOS 最低运行版本需以最终 Mach-O load commands 和目标系统启动结果核对，不能仅通过写 plist 声明支持。

## P8 需要的交接产物

1. 真实 v2.6.0 安装包、app 更新归档、latest.json、对应 `.sig` 与 SHA256；记录实际 platform keys 和 URL。
2. 同安装身份的 GPUI Windows NSIS、macOS app 更新归档（DMG 不能直接代替）、签名、元数据与最低 OS 路由方案。
3. 独立测试密钥/端点以及带测试公钥的旧客户端等价构建；另安排未经修改旧安装器的身份/数据兼容验证。
4. 更新签名和 Apple 应用签名/公证分别核验。正式私钥不放入原型、日志或文档。
5. Windows UAC 取消、坏签名、断网、解包失败及磁盘满的恢复记录；GPUI N→N+1 也需演练。
6. Gitee 重写前后产物字节与签名对应的核查、旧数据读写/回退副本、安装回退步骤。

当前完成源码身份清单、Windows 试验包、macOS 布局配方、真实更新元数据和交接列表；未执行真实旧客户端升级，未向正式 latest 上传任何产物。P0-06 的安装运行证据及 G8 仍待补齐。

## 已采集的真实 v2.6.0 元数据

2026-09-07 从固定 [v2.6.0 latest.json](https://github.com/Horbin-Magician/rotor/releases/download/v2.6.0/latest.json) 读取并原样保存至 [证据文件](evidence/v2.6.0-latest.json)。`version=2.6.0`，`pub_date=2026-08-04T07:18:33.580Z`。

- `windows-x86_64` 与 `windows-x86_64-nsis` 均指向 `Rotor_2.6.0_x64-setup.exe`，不是 ZIP。
- `darwin-aarch64` 与 `darwin-aarch64-app` 均指向 `Rotor_aarch64.app.tar.gz`，不是 DMG。
- 四项都有 signature 字段。这里只证明元数据形状与签名字段存在；尚未下载旧产物本体或执行密码学验签。

此样例用于 P8 的更新交接和兼容测试，不修改正式端点或元数据。
