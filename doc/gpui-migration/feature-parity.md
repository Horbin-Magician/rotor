# 功能与代码迁移对照

状态核对（2026-09-08，代码至 `75429c5`）：44 项已逐行补入当前实现位置、主要接入/修复 commit 和证据边界。“已接入”表示代码通路存在，不表示整行行为、视觉效果或发布验收已通过。Windows 是本轮重点，macOS 按用户要求暂缓；任何未测项都没有据此标记通过。

返回 [迁移计划](README.md)；测试编号对应 [验收文档](validation.md)。行为基线为 `40addee`；旧壳/核心入口链接按当前保留源码位置修正，原生实现另列。P9 搬迁后仍需更新路径。

## 1. 功能完成表

后续范围收口及本次发现的缺口见 [Windows 功能核对](evidence/windows-feature-audit.md)。

每行完成时补充实现 commit、对应验收证据和实际行为差异。未测平台不能记为通过。所有现有功能均须完成；原型可以只实现部分功能，正式版本不得保留占位按钮。

主要 commit 不一定包含整行的全部后续修复。E1 是历史实现/单测记录，E2/E3 只有已写明的窗口样本；它们均不能代替 V01–V21 的完整验收。

| ID | 基线能力与需保留的行为 | 旧壳/核心入口 | 目标归属 | 阶段/验收 | 当前原生实现 / 主要 commit | Windows 证据与未完成项 |
|---|---|---|---|---|---|---|
| L01 | 后台常驻；关闭普通窗口不退出；托盘设置/显式退出 | [lib.rs](../../src-tauri/src/lib.rs)、[tray.rs](../../src-tauri/src/integration/tray.rs) | desktop + platform | P2 / V01 | [main.rs](../../crates/rotor-desktop/src/main.rs)、[system.rs](../../crates/rotor-desktop/src/system.rs)；`b42229e` | 已接入；Windows 联合验收未完成 [E1] |
| L02 | Windows 启动尝试提权、单实例；二次启动不得重复索引/注册 | lib.rs、platform/sys_util.rs | desktop 启动协调 | P2 / V02 | [main.rs](../../crates/rotor-desktop/src/main.rs)、[platform/single_instance.rs](../../src-tauri/crates/rotor-platform/src/single_instance.rs)、[platform/legacy_instance.rs](../../src-tauri/crates/rotor-platform/src/legacy_instance.rs)；`ea80da0` | 开发/正式身份已接入；UAC/旧新实例实机未完成 [E1] |
| L03 | macOS 隐藏 Dock、窗口激活及跨 Spaces 覆盖 | lib.rs、[platform.rs](../../src-tauri/src/integration/screenshot_platform.rs) | platform 窗口策略 | P0/P2 / V03 | [platform/desktop.rs](../../src-tauri/crates/rotor-platform/src/desktop.rs)、[platform/overlay.rs](../../src-tauri/crates/rotor-platform/src/overlay.rs)；`fb8985f` | macOS 代码存在；本轮按用户要求暂缓运行验收 |
| L04 | 默认及自定义全局快捷键；Press/Release 去抖、陈旧按下状态恢复 | [application.rs](../../src-tauri/src/integration/application.rs) | runtime + global-hotkey adapter | P2 / V04 | [system.rs](../../crates/rotor-desktop/src/system.rs)、[runtime/shortcuts.rs](../../src-tauri/crates/rotor-runtime/src/shortcuts.rs)；`a7b8bf8` | 已接入；事务/去抖单测见 E1，真实冲突/睡眠未完成 [E1] |
| L05 | 冲突通知缓存、显示设置页及注册回滚 | application.rs、[core_cmd.rs](../../src-tauri/src/command/core_cmd.rs) | runtime 通知/事务 + UI | P2/P3 / V04/V07 | [system.rs](../../crates/rotor-desktop/src/system.rs)、[settings.rs](../../crates/rotor-ui/src/settings.rs)、[main.rs](../../crates/rotor-desktop/src/main.rs)；`f0131af` | 回滚及最近警告已接入；完整通知/失败交互待验 [E1] [E3] |
| L06 | 自启动启停、原生对话框、日志、打开 URL | lib.rs、[Setting.vue](../../src/pages/Setting.vue)、core_cmd.rs | platform + desktop | P2/P8 / V05/V18 | [platform/startup.rs](../../src-tauri/crates/rotor-platform/src/startup.rs)、[platform/desktop.rs](../../src-tauri/crates/rotor-platform/src/desktop.rs)、[logging.rs](../../crates/rotor-desktop/src/logging.rs)；`fb8985f` | 已接入；登录重启、对话框及卸载清理待验 [E1] |
| S01 | 所有配置读取、默认回填、持久化、旧键兼容 | [config.rs](../../src-tauri/crates/rotor-common/src/config.rs) | common + ConfigService | P1/P3 / V06 | [common/config.rs](../../src-tauri/crates/rotor-common/src/config.rs)、[common/profile_migration.rs](../../src-tauri/crates/rotor-common/src/profile_migration.rs)；`47a5cd7` | 事务和副本兼容有测试；旧安装版双向操作未完成 [E1] |
| S02 | 中英文、系统跟随/浅色/深色、系统跟随语言和当前枚举值 | [i18n.ts](../../src/plugins/i18n.ts)、[theme.ts](../../src/styles/theme.ts)、common/i18n.rs | common i18n + UI theme | P3 / V06/V17 | [visual.rs](../../crates/rotor-ui/src/visual.rs)、[settings.rs](../../crates/rotor-ui/src/settings.rs)、[common/i18n.rs](../../src-tauri/crates/rotor-common/src/i18n.rs)；`e69fe53` | 设置中英/浅深已有截图；系统跟随及全工具待验 [E2] [E3] |
| S03 | 概览内存、索引、权限和版本信息 | core_cmd.rs、[OverviewSettings.vue](../../src/components/setting/OverviewSettings.vue) | runtime 状态快照 + UI | P3 / V06 | [settings/overview.rs](../../crates/rotor-ui/src/settings/overview.rs)、[runtime/services.rs](../../src-tauri/crates/rotor-runtime/src/services.rs)；`f0131af` | 实际快照和索引详情已接入；完整视觉验收待补 [E3] |
| S04 | 快捷键录入、保存路径、缩放步长、排除目录、翻译配置 | [设置组件](../../src/components/setting)、[ShortcutInput.vue](../../src/components/common/ShortcutInput.vue) | UI + ConfigService | P3 / V06/V07 | [settings.rs](../../crates/rotor-ui/src/settings.rs)、[settings/actions.rs](../../crates/rotor-ui/src/settings/actions.rs)；`019d6d1` | 已接入；录制/IME/路径与失败回滚实机未完成 [E2] |
| Q01 | 快捷操作增删改、启停、规范化、稳定 ID 和执行 | [quick.rs](../../src-tauri/crates/rotor-runtime/src/quick.rs)、[quick_cmd.rs](../../src-tauri/src/command/quick_cmd.rs) | runtime quick | P3 / V07 | [settings/actions.rs](../../crates/rotor-ui/src/settings/actions.rs)、[runtime/quick.rs](../../src-tauri/crates/rotor-runtime/src/quick.rs)；`86af674` | 紧凑列表与编辑已接入；CRUD/执行联合验收未完成 [E2] |
| Q02 | 重复启用快捷键拒绝、系统注册失败/保存失败回滚 | quick.rs、quick_cmd.rs | runtime 事务 + platform 注册器 | P1/P3 / V07 | [runtime/services.rs](../../src-tauri/crates/rotor-runtime/src/services.rs)、[runtime/shortcuts.rs](../../src-tauri/crates/rotor-runtime/src/shortcuts.rs)；`a7b8bf8` | 事务失败单测有记录；系统级联合验收未完成 [E1] |
| Q03 | quick_actions_revision 兼容；Windows cmd /C、macOS sh -lc 命令语义 | quick.rs、config.rs | runtime + platform 进程启动 | P3 / V07 | [runtime/quick.rs](../../src-tauri/crates/rotor-runtime/src/quick.rs)、[common/config.rs](../../src-tauri/crates/rotor-common/src/config.rs)；`019d6d1` | revision 和平台命令语义保留；Windows 实际执行待验 [E1] |
| F01 | NTFS/普通卷索引、持久化缓存、监听增量、状态显示 | [file_data](../../src-tauri/crates/rotor-searcher/src/file_data) | searcher 核心 | P1/P4 / V08 | [searcher/file_data/mod.rs](../../src-tauri/crates/rotor-searcher/src/file_data/mod.rs)；`1c4a797` | 核心复用；原生真实索引/缓存/增量联合验收待补 [E1] |
| F02 | 中文拼音/首字母、通配符、别名、排序、图标和路径 | [search_match.rs](../../src-tauri/crates/rotor-searcher/src/file_data/volume/search_match.rs)、volume/* | searcher + UI | P4 / V08 | [searcher/file_data/volume/search_match.rs](../../src-tauri/crates/rotor-searcher/src/file_data/volume/search_match.rs)、[searcher.rs](../../crates/rotor-ui/src/searcher.rs)；`0751bac` | 匹配/别名有单测，固定语料及图标实机待验 [E1] |
| F03 | 查询更新/追加、结果上限 100、键盘选中、打开/管理员打开 | [useFileSearch.ts](../../src/features/searcher/composables/useFileSearch.ts)、[搜索组件](../../src/components/searcher) | runtime + SearcherView | P4 / V08 | [searcher.rs](../../crates/rotor-ui/src/searcher.rs)、[search_results.rs](../../crates/rotor-ui/src/search_results.rs)；`0751bac` | 100 项/去重/请求 ID 已接入；打开文件/分页实机待验 [E1] |
| F04 | 唤起更新索引、输入聚焦、窗口高度、失焦/Escape 隐藏与 release | [searcher/lib.rs](../../src-tauri/crates/rotor-searcher/src/lib.rs)、[useSearcherWindow.ts](../../src/features/searcher/composables/useSearcherWindow.ts) | desktop 窗口策略 + UI | P4 / V08 | [main.rs](../../crates/rotor-desktop/src/main.rs)、[searcher.rs](../../crates/rotor-ui/src/searcher.rs)、[searcher/file_data/mod.rs](../../src-tauri/crates/rotor-searcher/src/file_data/mod.rs)；`75429c5` | 关闭释放遗漏及缓存保留已修复；循环/内存实测未完成 [E6] |
| F05 | 排除规则支持目录名/绝对路径/home 路径；修改后重建 | [excluded_dirs.rs](../../src-tauri/crates/rotor-searcher/src/file_data/excluded_dirs.rs)、core_cmd.rs | searcher + ConfigService | P3/P4 / V06/V08 | [searcher/file_data/excluded_dirs.rs](../../src-tauri/crates/rotor-searcher/src/file_data/excluded_dirs.rs)、[settings.rs](../../crates/rotor-ui/src/settings.rs)；`70762f3` | 解析/保存后重建已接入；实际目录语料待验 [E1] |
| T01 | 输入翻译、清空/聚焦、Enter、结果复制、动态高度和失焦/Escape 隐藏 | [Translator.vue](../../src/pages/Translator.vue) | TranslatorView + desktop | P4 / V09 | [translator.rs](../../crates/rotor-ui/src/translator.rs)、[placement.rs](../../crates/rotor-desktop/src/placement.rs)；`86af674` | 动态高度/复制已接入；仅空输入窗口有截图，结果与 IME 待验 [E3] |
| T02 | 划词复制、等待修饰键释放、剪贴板轮询、文本恢复、光标跟随与边界限制 | [translator/lib.rs](../../src-tauri/crates/rotor-translator/src/lib.rs)、[selection.rs](../../src-tauri/crates/rotor-platform/src/selection.rs) | runtime selection + platform | P4 / V09 | [platform/selection.rs](../../src-tauri/crates/rotor-platform/src/selection.rs)、[translator/lib.rs](../../src-tauri/crates/rotor-translator/src/lib.rs)、[runtime/services.rs](../../src-tauri/crates/rotor-runtime/src/services.rs)；`11663bc` | 已接入；修饰键/并发复制/焦点实机组合待验 [E1] |
| T03 | Google、自动中英目标、显式目标语言 | [engine.rs](../../src-tauri/crates/rotor-translator/src/engine.rs) | translator 核心 | P4 / V09 | [translator/engine.rs](../../src-tauri/crates/rotor-translator/src/engine.rs)；`a2b5fc3` | 规则保留；旧源码 GUI 曾遇 429，原生真实引擎待验 [E2] |
| T04 | DeepSeek key/model、started/delta 流、完成结果、错误和超时 | engine.rs、[translator_cmd.rs](../../src-tauri/src/command/translator_cmd.rs)、[types.ts](../../src/features/translator/types.ts) | translator + runtime + UI | P4 / V09 | [translator/engine/stream.rs](../../src-tauri/crates/rotor-translator/src/engine/stream.rs)、[translator.rs](../../crates/rotor-ui/src/translator.rs)；`9928c12` | 流与异常结束已接入；UTF-8/EOF 单测通过，真实引擎 GUI 待验 [E4] |
| T05 | 自定义 URL 的 text/from/to/key 模板、编码与响应解析；旧响应隔离 | engine.rs、Translator.vue | translator + request ID | P4 / V09 | [translator/engine.rs](../../src-tauri/crates/rotor-translator/src/engine.rs)、[runtime/services/translation_tests.rs](../../src-tauri/crates/rotor-runtime/src/services/translation_tests.rs)；`4dadf59` | 实际 localhost 身份/取消/503 测试通过；GUI 待验 [E4] |
| C01 | 所有显示器捕获、显示器拓扑变更、遮罩就绪后显示 | [screenshot/lib.rs](../../src-tauri/crates/rotor-screenshot/src/lib.rs)、[monitor.rs](../../src-tauri/crates/rotor-screenshot/src/monitor.rs) | screenshot + desktop | P5 / V10/V12 | [capture.rs](../../crates/rotor-desktop/src/capture.rs)、[screenshot/monitor.rs](../../src-tauri/crates/rotor-screenshot/src/monitor.rs)；`6c47a9c` | 已接入；双屏/混合 DPI/热插拔会话未完整验收 [E1] [E3] |
| C02 | 会话有效性/恢复、finish 与 cancel、旧回调失效、异常清理 | screenshot/lib.rs、[Mask.vue](../../src/pages/Mask.vue) | CaptureSession 状态机 | P5 / V11 | [screenshot/session.rs](../../src-tauri/crates/rotor-screenshot/src/session.rs)、[capture.rs](../../crates/rotor-desktop/src/capture.rs)；`6ac493a` | 状态机与过期代数已接入；实际取消/恢复循环待验 [E1] |
| C03 | 自动窗口框选、拖选、遮罩、尺寸提示、放大镜和颜色取样 | Mask.vue、[mask 组件](../../src/components/screenShotter/mask)、screen_shotter_cmd.rs | MaskView + 几何模块 | P5 / V10 | [capture.rs](../../crates/rotor-ui/src/capture.rs)、[screenshot/img_util.rs](../../src-tauri/crates/rotor-screenshot/src/img_util.rs)；`bc2e9e9` | 窗口/轮廓细化及选区 UI 已接入；逐屏交互待验 [E1] |
| C04 | C 复制颜色、Escape 取消、跨显示器焦点移动 | Mask.vue、screenshot/platform.rs | UI + desktop + clipboard | P5 / V10/V11 | [capture.rs](../../crates/rotor-ui/src/capture.rs)、[capture.rs](../../crates/rotor-desktop/src/capture.rs)；`20dcf0c` | 取色/取消/焦点已接入；边缘位置有单测，桌面组合待验 [E2] |
| C05 | RGBA 缓存、预建贴图/临时缓存、数据延迟到达后的就绪协调 | [capture_cache.rs](../../src-tauri/crates/rotor-screenshot/src/capture_cache.rs)、[screenshot_data.rs](../../src-tauri/src/integration/screenshot_data.rs) | screenshot ImageStore + UI image adapter | P1/P5 / V11/V12 | [capture.rs](../../crates/rotor-desktop/src/capture.rs)、[pins.rs](../../crates/rotor-desktop/src/pins.rs)、[capture.rs](../../crates/rotor-ui/src/capture.rs)；`083f830` | 原生图像句柄替换旧 IPC/cache label；就绪时序实机待验 [E1] |
| P01 | 新建、恢复、置顶、拖动、最小化、关闭贴图 | [Pin.vue](../../src/pages/Pin.vue)、screenshot/lib.rs | PinView + desktop | P6 / V13 | [pins.rs](../../crates/rotor-desktop/src/pins.rs)、[pin.rs](../../crates/rotor-ui/src/pin.rs)；`6c47a9c` | 单张合成贴图恢复可见已由用户确认；完整交互待验 [E3] |
| P02 | 滚轮缩放、zoom_delta、跨屏缩放、工具栏显隐和提示 | Pin.vue、[PinToolbar.vue](../../src/components/screenShotter/pin/PinToolbar.vue) | UI + 几何模块 | P6 / V13 | [pin.rs](../../crates/rotor-ui/src/pin.rs)；`86af674` | 缩放/底部工具栏已接入；跨屏/窄图显隐待验 [E2] |
| P03 | 八方向边缘裁剪调整、最小选区尺寸、源图边界和窗口位置联动 | Pin.vue、PinEdgeGlow.vue | canvas 几何 + PinView | P6 / V13 | [pin/crop.rs](../../crates/rotor-ui/src/pin/crop.rs)、[crop.rs](../../crates/rotor-canvas/src/crop.rs)；`ffe2b9f` | 几何和指针捕获已接入，边界单测通过；真实八方向拖动待验 [E1] |
| P04 | PNG 保存、路径询问/记忆；成功后关闭，取消/失败保留 | Pin.vue、[screen_shotter_cmd.rs](../../src-tauri/src/command/screen_shotter_cmd.rs) | ExportService + platform | P6 / V14 | [pin.rs](../../crates/rotor-ui/src/pin.rs)、[runtime/pins.rs](../../src-tauri/crates/rotor-runtime/src/pins.rs)；`76f0352` | 保存快照与成功/失败路径已接入；对话框/磁盘错误实机待验 [E1] |
| P05 | 复制合成图像，成功后关闭；S/Escape/Enter/H 等可配局部键 | Pin.vue、config.rs | UI action + clipboard | P6 / V14/V17 | [pin.rs](../../crates/rotor-ui/src/pin.rs)、[platform/clipboard.rs](../../src-tauri/crates/rotor-platform/src/clipboard.rs)；`019d6d1` | 局部按键/复制已接入；剪贴板互通及 IME 实机待验 [E1] |
| P06 | 保存/恢复位置、选区、image_rect、zoom、minimized；删除记录与图像 | [shotter_record.rs](../../src-tauri/crates/rotor-screenshot/src/shotter_record.rs) | screenshot persistence | P6/P8 / V15/V18 | [screenshot/pin_store.rs](../../src-tauri/crates/rotor-screenshot/src/pin_store.rs)、[common/profile_migration.rs](../../src-tauri/crates/rotor-common/src/profile_migration.rs)；`47a5cd7` | 合成副本/原点兼容有证据；真实安装版读写和坏数据恢复待验 [E1] [E3] |
| A01 | 画笔、矩形、箭头、文字、进入/退出编辑 | [PinCanvas.vue](../../src/components/screenShotter/pin/PinCanvas.vue)、PinDrawingToolbar.vue | rotor-canvas + UI | P6 / V14 | [pin/annotation.rs](../../crates/rotor-ui/src/pin/annotation.rs)、[document.rs](../../crates/rotor-canvas/src/document.rs)；`389f1a6` | 文档/工具已接入；实际标注操作待验 [E1] |
| A02 | 中文文字输入确认/取消、样式与尺寸、撤销 | PinTextInput.vue、PinCanvas.vue | UI input + AnnotationDocument | P6 / V14/V17 | [pin/annotation.rs](../../crates/rotor-ui/src/pin/annotation.rs)、[renderer.rs](../../crates/rotor-canvas/src/renderer.rs)；`389f1a6` | 中文字体/确认/撤销已接入；实际中文 IME 待验 [E1] |
| A03 | 屏幕显示与 PNG/剪贴板/OCR 输入合成一致，保留裁剪/zoom 导出语义 | PinCanvas.vue、Pin.vue | rotor-canvas 离屏合成 | P6 / V14 | [renderer.rs](../../crates/rotor-canvas/src/renderer.rs)、[pin/annotation.rs](../../crates/rotor-ui/src/pin/annotation.rs)、[pin.rs](../../crates/rotor-ui/src/pin.rs)；`76f0352` | 合成与导出快照已接入；旧 zoom×DPI 矩阵未冻结 [E1] |
| O01 | OCR 模型加载、线程隔离、结果合并、缓存中毒恢复与 30 秒空闲释放 | [img_util.rs](../../src-tauri/crates/rotor-screenshot/src/img_util.rs) | screenshot OCR service | P7 / V16 | [screenshot/img_util.rs](../../src-tauri/crates/rotor-screenshot/src/img_util.rs)、[runtime/services.rs](../../src-tauri/crates/rotor-runtime/src/services.rs)；`331ee7d` | Windows 合成识别/空闲释放已有记录；全场景验收未完成 [E1] |
| O02 | OCR 覆盖层、文字选择复制、缩放定位和模式切换 | [PinOcrOverlay.vue](../../src/components/screenShotter/pin/PinOcrOverlay.vue)、Pin.vue | OcrOverlay + PinView | P7 / V16/V17 | [pin/ocr.rs](../../crates/rotor-ui/src/pin/ocr.rs)；`331ee7d` | 覆盖层选择/复制已接入；鼠标、键盘和缩放定位待验 [E1] |
| O03 | 识别失败/空结果反馈、重复请求控制、图像变化后缓存失效 | img_util.rs、Pin.vue | runtime + UI | P7 / V16 | [pin/ocr.rs](../../crates/rotor-ui/src/pin/ocr.rs)、[pin/annotation.rs](../../crates/rotor-ui/src/pin/annotation.rs)；`20dcf0c` | 请求/图像版本隔离及空结果反馈已接入；实际模式切换待验 [E1] [E2] |
| R01 | 版本提示、更新检查、确认、下载进度、安装和重启 | [Setting.vue](../../src/pages/Setting.vue)、[UpdateModals.vue](../../src/components/setting/UpdateModals.vue) | UpdateService + UI | P8 / V19 | [settings/updates.rs](../../crates/rotor-ui/src/settings/updates.rs)、[runtime/updates.rs](../../src-tauri/crates/rotor-runtime/src/updates.rs)、[updater](../../crates/rotor-updater/src/lib.rs)；`4491384` | Windows 交接代码已接入；真实升级/故障恢复未验收 [E5] |
| R02 | Windows NSIS per-machine、语言和图标；macOS app/DMG | [平台配置](../../src-tauri/tauri.conf.json)、tauri.*.conf.json | packaging + xtask | P8 / V18 | [windows.nsi](../../native/windows.nsi)、[installer.rs](../../xtask/src/installer.rs)；`07c1b8f` | Windows 历史包已构建；安装/覆盖/卸载未验收，macOS 暂缓 [E5] |
| R03 | GitHub 签名更新产物与 Gitee latest.json/附件同步 | [publish.yml](../../.github/workflows/publish.yml)、[sync-to-gitee.yml](../../.github/workflows/sync-to-gitee.yml) | CI + UpdateService | P8 / V19 | [native-candidate.yml](../../.github/workflows/native-candidate.yml)、[prepare-gitee-release.py](../../.github/scripts/prepare-gitee-release.py)；`d21dd91 / 1409bc2` | 部分：候选/签名/镜像代码就绪；正式发布流程及线上联调未完成 [E5] |
| R04 | Rust 应用版本、OCR/图标资源、运行库、日志位置 | [bump-version.mjs](../../scripts/bump-version.mjs)、lib.rs、assets/model | xtask + ResourceLocator | P8/P9 / V18/V21 | [versions.rs](../../xtask/src/versions.rs)、[builder.rs](../../xtask/src/builder.rs)、[common/resources.rs](../../src-tauri/crates/rotor-common/src/resources.rs)；`6f7cc1e` | 版本/资源/收据已接入；最终路径清理和干净安装构建未完成 [E5] |

## 2. IPC 与事件去向

[E1]: progress.md "历史实现和测试记录"
[E2]: evidence/ui-modernization.md "旧/新 UI 对照与部分窗口截图"
[E3]: evidence/windows-ui-followup.md "Windows 续作、恢复观察和显示拓扑"
[E4]: evidence/windows-translation-stream.md "流式结束及真实 localhost 请求测试"
[E5]: candidate-status.md "历史候选构建与未通过的发布门槛"
[E6]: evidence/windows-search-release.md "搜索关闭释放及缓存回归测试"

这里记录兼容映射，不要求 GPUI 继续使用原 IPC 名称。GPUI 调用 Rust 用例接口，旧 Tauri adapter 在过渡阶段调用同一接口。

| 原命令/通路 | 新去向或消除方式 |
|---|---|
| get_cfg、get_all_cfg、set_cfg | ConfigService 快照和校验事务；设置变更事件触发 UI/索引/热键副作用 |
| get_app_version、get_overview_info | 应用版本元数据、runtime 概览快照 |
| take_shortcut_registration_notices、shortcut-registration-conflict | 带稳定 ID 的通知队列；视图未创建时保留，创建后消费 |
| open_url | platform URL 打开能力，保留原有输入验证 |
| get_quick_actions、set_quick_actions、run_quick_action | runtime QuickActionService；规则、注册、保存一次事务 |
| searcher_find、searcher_release、searcher_index_status、update_result、index-state-changed | SearchService 请求与有 QueryId 的替换/追加结果、索引状态快照/通知；显式 release |
| open_file、open_file_as_admin | platform 文件操作服务 |
| translator_translate、Channel<TranslateStreamEvent> | TranslationService 流 + 最终结果；started/delta 顺序和旧请求隔离 |
| translate-select、translate-input | desktop 触发 TranslatorView 的两种启动模式 |
| get_screenshot_data、get_screenshot_data_shared | 同进程只读图像句柄；移除 binary IPC/WebView2 SharedBuffer/JS 超时与 fallback |
| get_screen_rects、change_current_mask | screenshot/window rect 数据、desktop 活动显示器和焦点策略 |
| is_screenshot_session_current、get_recoverable_screenshot_session | CaptureSession 当前版本与 ready/capture 有效性查询 |
| finish_screenshot_session、cancel_screenshot_session | 会话状态机结束路径；finish/cancel 的缓存保留差异需保持 |
| show-mask、hide-mask | 带 CaptureSessionId 的桌面窗口更新 |
| new_pin、show-pin | PinService 创建/恢复记录并通知对应窗口 |
| new_cache_pin、close_cache_pin、clear_screenshot_cache | 应用控制的贴图预热和图像资源生命周期；旧 label 仪式消除 |
| get_pin_state、update_pin_state、update_pin_selection | 类型化 PinState 快照、布局/选区持久化 |
| delete_pin_record | PinService 删除事务；关闭与最小化行为区分 |
| save_img | 离屏导出 → 路径选择 → 文件写入 → 成功/取消/错误结果 |
| img2text | OCR service 处理合成图像，返回可区分空结果和错误的状态 |

迁移 `src-tauri/src/lib.rs` 注册的全部 34 个命令时应逐个核对。注册表中的命令存在不代表新 UI 一定需要同名接口；合并或删除通路必须在这里保留去向说明。

## 3. 配置及磁盘格式

| 配置组 | 需保持的键与值含义 |
|---|---|
| 语言/主题 | language、theme；保留系统跟随及现有数值字符串含义，不按新组件枚举重新解释 |
| 文件保存 | save_path、if_auto_change_save_path、if_ask_save_path |
| 贴图 | zoom_delta、current_workspace、shortcut_pinwin_save/close/copy/hide |
| 全局热键 | shortcut_search、shortcut_screenshot、shortcut_translate_select、shortcut_translate_input |
| 搜索 | search_excluded_dirs；默认排除列表和路径展开规则 |
| 翻译 | translator_engine、translator_target_lang、translator_deepseek_api_key、translator_deepseek_model、translator_custom_url、translator_custom_key |
| 快捷操作 | quick_actions（JSON 字符串）、quick_actions_revision（当前默认 2） |

- Windows 当前默认快捷操作只有 Terminal（Ctrl+Shift+T，`start wt`）；macOS 有 Terminal 和 Finder。不要按旧说明在 Windows 自动补一个 Ctrl+Shift+E 动作。
- 全局热键默认前缀为 Windows Ctrl+Shift / macOS Cmd+Shift，搜索 F、截图 S、划词 D、输入 W；贴图局部默认为 S、Escape、Enter、H。
- DeepSeek 默认模型名称沿用基线 `deepseek-v4-flash`；迁移不隐式改变用户模型或发送真实试译请求。
- 配置持久化仍可用原 TOML Map，内部用类型化 getter 校验，不删除未知键。不在日志、测试产物或提交的样例中包含用户 API key。
- 贴图记录字段：monitor_pos、monitor_size、rect、可选 image_rect、offset、zoom_factor、mask_label、minimized；外层 workspaces/default/shotters 结构保持兼容。
- 搜索缓存可重建，但首次不改变其格式。若不可避免更改，按版本隔离并重建索引，不删除配置或截图。
- 标注当前属于画布内存状态；不把“重启后可编辑标注”列为现有功能。新合成流程应保持当前保存/复制结果，不顺手扩大持久化范围。

## 4. 当前行为与需要单独确认的改进

下列项目必须记录成“保持/修复/延后”，不可在迁移中悄悄改变：

| 项目 | 基线边界 | 迁移策略 |
|---|---|---|
| 划词剪贴板恢复 | 保存并恢复文本；没有完整多格式快照；并发用户复制可能被覆盖 | 首先保留已支持场景；新增 change count/所有权检查避免覆盖较新的用户复制，测试明确；完整富文本/图像恢复单独评估 |
| 导出像素尺寸 | Konva 按窗口 scaleFactor 和当前 zoom 导出 | P0 建立样例，P6 默认保持；固定源图尺寸作为单独行为决策 |
| 配置落盘失败 | AppConfig 修改内存后保存，失败时存在局部不一致风险 | P1 把“失败后整体恢复”作为可靠性修复，增加错误注入测试 |
| 损坏贴图记录 | 原实现可能重建/清理无效记录 | GPUI 读取前备份；正常/损坏样例分开验证，避免误删 |
| 标注后 OCR 缓存 | 旧 UI 有结果缓存，图像变更边界需要核对 | 使用图像 revision 防止旧文字覆盖新截图，作为明确修复 |
| 模态框导致 blur | 搜索/翻译采用失焦隐藏；GPUI 弹层可能产生不同事件顺序 | 将 owned dialog/IME 与真正应用失焦区分，保留可继续操作的预期 |
| 当前工作区字段 | 配置/记录有工作区结构，现有实现主要使用 default | 保留数据；本次不新建工作区管理 UI |
| 保存操作步数 | 旧设置多数通过 watch 即时落盘；快捷操作编辑已有保存，启停/删除直接提交 | 原生选项即时事务保存，但文本/快捷键、快捷操作启停/删除仍需额外点“保存更改”；与 UI-04“不增加常用操作步骤”的要求尚需收口，不能把双方已有的编辑保存误报为新增步骤 |
| redo 范围 | P6 计划明确首版不新增 redo，基线只要求 undo | 已移除额外 redo 按钮和 Ctrl/Cmd+Shift+Z；内部 redo 仅用于失败事务恢复。撤销仍需实际标注/IME 验收 |
| 工具窗口重建 | 旧搜索/翻译隐藏并复用 WebView | 原生关闭后按需重建；搜索关闭 release 已补回，热唤起性能及输入状态仍需实测 |

## 5. 资源和发布清单

- OCR 模型：`pp-ocrv6_tiny_det.onnx`、`pp-ocrv6_tiny_rec.onnx`、`ppocrv6_tiny_dict.txt`。实际二进制运行库清单通过 release 包的依赖检查确定，不能只拷贝模型文件。
- 图标：窗口/安装器/托盘所需 ICO、ICNS、PNG；macOS 白色托盘资源及主题适配行为保留。字体仅在系统回退不足时按许可打包。
- 应用身份：产品 Rotor、bundle ID `cc.fluctus.rotor`。开发包另用可区分身份；正式升级要核对 Windows 卸载注册项、安装目录与 macOS bundle 路径。
- 更新：保留 Gitee/GitHub 地址的顺序、签名公钥信任和旧客户端可识别的目标键/产物格式。新 metadata 协议如有变化须兼容旧客户端或提供过渡版本。
- Windows NSIS：per-machine、简体中文/英文、图标、开始菜单/卸载、自启动残留清理；GPUI 包不再引导安装 WebView2，但不能卸载其他程序共享的 WebView2 Runtime。
- macOS：app/DMG、资源路径、屏幕录制/辅助功能权限、Dock 行为、实际采用的签名和公证流程。更新包签名与 Apple 应用签名分别验证；可以保持既有分发方式，但不能用本地可运行替代安装包验收或宣称已公证。
- 版本修改由 Rust xtask 的 `set-version` 管理，旧 Node 版本脚本已改为代理入口；commit/tag/push 不由版本修改隐式触发。旧 Tauri/Node 发布 workflow 仍保留，最终切换待 P9 门槛满足后实施。
