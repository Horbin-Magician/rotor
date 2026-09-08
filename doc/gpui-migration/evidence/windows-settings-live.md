# Windows 设置实机验收（2026-09-08）

用户恢复桌面验收授权后，使用 `6efe337` 开发身份暂存程序及新的 `target/ui-live-6efe337` 合成配置。启动参数为 `--no-elevate --no-index --no-hotkeys --data-dir D:/Project/rotor/target/ui-live-6efe337`；没有凭据、快捷操作命令或贴图，未访问正式配置或安装程序。

首次测试启动使用了 Hidden 启动参数，进程存活且有恢复日志，但窗口工具未发现设置页；核对测试实例身份后更正为 Normal 可见启动，设置页随即可枚举和操作。此启动参数问题不记作应用窗口验收失败。

## 已实际执行

1. 在 DeepSeek 模型字段输入 `fixture-自动保存 Aa 0123`，未点击保存、未离开字段，界面显示“设置已保存”，磁盘值一致。[截图](ui-windows/native-autosave-success-6efe337.png)。这里只修改合成模型名，没有发起 DeepSeek 请求。
2. 将该合成 `config.toml` 设为只读，再追加 `-readonly-failure`。界面显示字段“未保存”和拒绝访问错误，磁盘仍为旧值；点击关闭后，窗口和草稿均保留。[截图](ui-windows/native-autosave-failed-close-6efe337.png)。
3. 解除只读后点击“保存更改”，错误及未保存标记消失，磁盘完整保存新值。
4. 再次设为只读并追加 `-discard-only`，点击“放弃未保存并关闭”，窗口关闭。解除只读并以同一隔离配置二次启动后设置窗口重新出现，翻译字段仍是之前已保存的值，没有 `-discard-only`。本轮结束时配置已恢复可写。
5. 贴图保存快捷键手输 `Ctrl+`，保持焦点时没有写入配置；按 Enter 后校验失败，字段和错误可见。[截图](ui-windows/native-shortcut-invalid-6efe337.png)。
6. 在同一字段启动录制并按 F8，录制结束、错误清除，配置出现 `shortcut_pinwin_save = "F8"`。此进程禁用了系统全局注册，不能据此宣称全局快捷键冲突验收通过。
7. 模型字段通过真实字母键和当前中文输入法输入拼音。先输入的英文 `n` 正常保存；切到中文模式后，`n`/`ni` 显示候选窗和组字文本，磁盘仍停留在之前已确认的英文 n，没有写入临时拼音。[组字截图](ui-windows/native-ime-preedit-6efe337.png)。
8. 按空格确认候选“你”，界面与磁盘均保存该中文；随后输入 h 再按 Escape，候选窗关闭，设置窗口保留，已保存内容未改变。[确认](ui-windows/native-ime-confirmed-6efe337.png)、[取消](ui-windows/native-ime-cancelled-6efe337.png)。最后恢复验收前的英文输入模式。这里证明当前系统中文输入法的候选/确认/取消，未通过工具独立确认具体 IME 产品身份。

## 发现与后续

非法快捷键原先显示英文并暴露 `shortcut_pinwin_save` 内部键名，已在 `0253418` 修复为随当前界面语言显示的字段名及重新录制/输入完整组合的说明。runtime 34 项测试、native/runtime 严格 clippy 通过，随后 release 构建成功。新程序在同一隔离配置中复验，[英文](ui-windows/native-shortcut-error-english-0253418.png)和[中文](ui-windows/native-shortcut-error-chinese-0253418.png)提示均完整可见；语言设置保存成功后，其他字段的失败状态仍保留，未误报全部保存成功。

修复版 release SHA-256：`6945d4d5d92c86e151cdb9793d2d018420d1a62a595e01041da464d1a789786f`。此次 UI 复验使用显式 `--resource-dir D:/Project/rotor/src-tauri/assets`，不是新安装包的资源验收；此前 `6efe337` 安装包不包含这份提示修复。

另完成设置页的[深色/英文组合](ui-windows/native-settings-dark-english-6efe337.png)，选择值即时更新界面和磁盘。通过窗口实际右/下边框拖动至截图尺寸 496×401，[通用设置](ui-windows/native-settings-small-dark-6efe337.png)可操作，[快捷键列表](ui-windows/native-shortcuts-small-bottom-6efe337.png)可滚动至最后一项，保存及关闭区仍可见。该样本未覆盖其余页面的窄窗口或不同 DPI；不能替代完整新旧对照矩阵。

本轮覆盖设置字段与局部快捷键的一组实机成功/失败/恢复操作，以及中文组字的确认/取消。没有完成输入法全矩阵、应用退出期间待保存事务、全局热键注册、贴图/OCR/翻译窗口、多屏/DPI、安装更新或性能验收；UI-01–UI-05 与 V17 的完整范围继续保留未完成。macOS 暂缓。

## 工具窗口续测

切换到新的 `target/ui-tools-0253418` 合成贴图配置，使用同一 `0253418` release 程序、本地 `127.0.0.1:18765` 固定翻译响应服务，并启用开发模式的 Alt 全局快捷键。日志报告一张合成贴图恢复并准备完成；窗口工具的 list_windows/list_apps 仍只暴露设置页，没有贴图窗口标识，因此未据日志认定贴图操作通过。

Ctrl+Alt+Shift+W 确实打开了[输入翻译浮窗](ui-windows/native-translation-empty-0253418.png)。工具可以取得浮窗附属截图，但没有返回独立窗口标识；其自动激活目标窗口的输入路径会与翻译失焦关闭行为冲突，参见此前 Windows UI 续作记录。本次已转请用户用固定 `short` 样例辅助验证实际结果，本地 health 返回 ready 不作为翻译结果展示通过的依据。
