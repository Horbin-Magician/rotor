# 原生 UI 对照与现代化验收

日期：2026-09-08；起点：`3cb21b6`。本记录区分源码对照、实际窗口观察和验收通过。

## 旧 UI 对照（UI-01）

| 范围 | 旧实现与有效习惯 | 原生改进要求 / 操作核对 |
|---|---|---|
| 设置 | `src/pages/Setting.vue`：左侧导航，右侧独立滚动；`settings.css`：13/20px 正文、14px 分组标题、12px 分组间距 | 保留左导航；标题、说明、表单和固定保存反馈分层；中英入口全部可见；切页不丢草稿 |
| 概览 | `OverviewSettings.vue`：内存/索引/权限摘要，详情行、状态图标、刷新及加载反馈 | 摘要卡和详情分组；加载/失败明确；目录长文本换行 |
| 快捷操作 | `QuickSettings.vue`：每项名称、组合键、命令、启停/运行/删除 | 原生每个输入明确标签；录制和保存可达；草稿与可运行状态区分 |
| 更新 | `UpdateModals.vue`、`GeneralSettings.vue`：版本、更新说明、进度和取消 | 集中更新页保留进度、取消、失败重试；主动作突出 |
| 搜索 | `SearchInput.vue`、`SearchResultItem.vue`：搜索图标、文件图标、名称/路径层级，方向键及 Enter | 主题感知选中态、路径弱化、空查询/无结果/加载/失败区分；虚拟列表保留 |
| 翻译 | `src/pages/Translator.vue`：输入、语言方向、流式正文、复制成功反馈；Enter 提交 | 输入和结果分区、语言方向、复制反馈、可读行距；Enter/Shift+Enter/IME/Escape 保留 |
| 截图 | `src/pages/Mask.vue` 与 `src/components/screenShotter/`：框选、取色、尺寸与放大镜 | 高对比边框与尺寸提示；工具不进入导出；屏幕边缘和 DPI 核对 |
| 贴图/标注/OCR | `src/pages/Pin.vue` 与 screenshot 组件：浮动图标工具、工具提示、裁剪/画笔/文字/撤销、OCR 选择复制 | 避免字母冒充实际快捷键；统一图标和选中态；小图/窄图工具可达；中文输入不触发贴图快捷键 |

旧颜色来自 `src/styles/theme.ts`：蓝色主色 `#54a4db`，浅色白/灰分层，深色 `#121212/#1f1f1f`。原生沿用蓝色识别感，使用 gpui-component 的语义主题色，避免固定半透明蓝色在两种主题中失去对比。

## 视觉规范（UI-02 实施依据）

- 使用现有系统字体及打包中文回退；正文 14px，辅助 12px，页面标题 24px；长路径和错误允许换行。
- 4px 间距基准；控件间距 8px，卡片间距 16px，正文边距 24px；卡片 12px 圆角、1px 语义边框。
- 设置左侧导航固定宽度，正文可收缩并独立滚动，保存及反馈留在底部；小窗口采用紧凑边距。
- 背景、卡片、次级文字、选中、错误均用主题语义色；选中不使用禁用态；只有暂不可执行的动作才禁用。
- 主要动作使用 primary，次要动作使用默认/ghost；图标附本地化 tooltip；输入使用组件原有焦点/IME/Tab 行为。
- 不增加持续动画或后台刷新；搜索保持虚拟列表，截图像素不受界面主题影响。

## 实际取样及待验收

Windows computer-use 已初始化并两次列举窗口。启动已安装 `C:/Program Files/Rotor/rotor.exe` 返回 `launched app did not expose a targetable window: cc.fluctus.rotor`，复查没有 Rotor 窗口。尚未取得旧版截图，源码对照不等于 UI-01/UI-05 完成。

后续逐模块记录：版本/commit、OS、窗口尺寸、语言、主题、DPI、固定内容、操作步骤、截图路径、通过/失败/未测。最少覆盖中文/英文 × 浅/深色，系统跟随、窄窗口、100/150/200% DPI；旧新使用同一内容。完整视觉/交互验收前，UI-01–UI-05 继续保持未关闭。

## 设置页第一批实现

- `rotor-ui/src/visual.rs` 提供语义主题卡片、标题及辅助文字；设置采用左导航、可滚动正文、固定保存/反馈区，并在 760px 以下缩小导航与边距。
- 选中导航和选项使用组件 selected 状态；保存按钮使用 primary；概览分为运行摘要、数据目录和权限卡片，刷新期间禁用重复请求，收到结果解除等待。
- 快捷操作补名称/快捷键/命令标签，操作行可换行；更新状态建立标题层级、错误使用语义色。
- `cargo check -p rotor-ui`、`cargo check -p rotor-desktop`、`cargo build -p rotor-desktop` 通过。debug 链接保留 MSVC 导入库提示 warning；未将构建作为视觉验收。
- 启动 debug 程序使用隔离资料目录 `target/ui-modernization-profile`，参数 `--no-elevate --no-index`。随后 computer-use 返回用户按物理 Escape 停止操作，本轮停止界面控制，未取得新窗口截图。
- 待继续：实际布局/窄窗口/双主题验证、搜索与翻译、截图与贴图工具美化，以及完整旧新对照。UI-01–UI-05 均未关闭。

## 搜索和翻译第一批实现

- 搜索改用主题选中/悬停色、文件图标回退及名称/路径层级，底栏显示结果数和键盘提示；明确区分空输入、搜索中、无结果、正在打开及失败。保留 100 项上限和虚拟列表。
- 翻译补回 Started/Finished 事件中的语言方向，结果区采用独立底色和 1.65 行高；复制显示成功反馈，流式追加/新请求重置反馈。空输入禁用提交，输入事件刷新该状态，保留 IME 确认保护。
- `cargo check -p rotor-ui`、12 项 `cargo test -p rotor-ui --lib` 及 `cargo clippy -p rotor-ui --all-targets --no-deps -- -D warnings` 通过。包含依赖的 clippy 在既有 `rotor-platform/src/installer.rs:27` 的 `manual_is_multiple_of` 上失败，尚待单独收敛。
- 本批没有进行桌面操作或实际新旧截图验收；视觉与交互门槛继续未通过。

## 截图、贴图与 OCR 第一批实现

- 贴图主操作使用图标和本地化提示，提示附实际配置的快捷键；画布工具使用本地化名称和 selected/toggled 状态，撤销/重做/完成采用统一图标。
- 工具栏采用主题背景、边框和阴影，按窗口高度滚动；空提示不占行，画布错误使用语义错误色。窄小贴图的遮挡与可达性仍需实际交互验收，不能仅据可滚动实现认定解决。
- OCR 空结果与执行错误分开显示，识别后提示拖选/双击操作，复制文字后显示反馈。
- 取色放大镜增加中英文和复制反馈；临近右/下边缘时换到光标另一侧，避免直接覆盖采样点。位置单测覆盖 600/1080/1440 逻辑长度的边缘和中部，以及小于提示卡的视口。
- native check、13 项 UI 测试及 desktop/ui `--all-targets --no-deps -- -D warnings` clippy 通过。
- 用户将 macOS 相关工作暂缓，本轮只推进 Windows。此前隔离测试进程仍存活，但没有可控制窗口，二次激活也未出现；结束该测试进程以重新构建与采集诊断，未操作正式安装或用户资料。

## Windows 实际设置窗口取样与修正

使用 `--no-elevate --no-index --data-dir target/ui-modernization-profile` 的 debug 原生程序。初次以 Hidden 方式启动未得到可控窗口；Normal 启动取得真实设置窗口，stderr 为空。窗口客户端按源码 820×600 逻辑像素创建，捕获图片 822×630（含原生标题栏/边界）；尚未单独记录显示器 DPI，不把这些截图作为 100/150/200% DPI 矩阵。

| 证据 | 实际观察 |
|---|---|
| [中文深色概览](ui-windows/native-overview-dark-20dcf0c.png) | 概览摘要/数据目录卡片和独立滚动区可见 |
| [英文深色设置](ui-windows/native-general-en-dark-20dcf0c.png) | 切换 English 后导航及标题更新，底部出现 Settings saved |
| [英文浅色设置](ui-windows/native-general-en-light-20dcf0c.png) | 点击 Light 后窗口正文切换浅色，保存反馈保留 |
| [快捷键页修正前](ui-windows/native-shortcuts-before-density-fix.png) | 三行卡片造成一屏仅约两项半；已据此改为宽窗口横向标签/输入/录制，窄窗口换行 |
| [调整后的浅色概览](ui-windows/native-overview-en-light-refined.png) | 14px 基准字体、蓝色强调与较紧凑摘要，卡片和正文层级清楚 |

新主题保存在 gpui-component 的 light/dark ThemeConfig 中，因此后续模式切换和系统跟随仍使用同一套配色。普通文本颜色对比度计算：浅色正文 16.30、辅助 4.67、选中 5.00、主按钮 5.81；深色分别 14.54、8.28、4.54、7.48。计算仅覆盖列出的不透明颜色对，不替代全部控件/禁用态的实际验收。

本批同时补图标按钮无障碍名称、切换按钮 toggled 状态，以及快捷操作输入变化时刷新“可运行”状态。native build 和 desktop/ui 的严格 `--no-deps` clippy 通过。旧 `yarn build` 与 `cargo build -p rotor --features tauri/custom-protocol` 通过，旧壳保留既有 unused/dead_code 和链接提示；未启用浏览器调试。旧壳尚未启动采图。

窗口输入检测到用户正在操作后，多次拒绝过期坐标；仅刷新读取状态，没有复用旧坐标。后续仍需快捷键密度修正后的截图、其余模块、窄窗口、IME、DPI 和逐项旧新对照，UI-01–UI-05 继续未关闭。

## 原生依赖链 lint 收敛

`rotor-platform` 的安装资源指针对齐检查改用 `is_multiple_of(2)`，`rotor-searcher` 的缺失父目录返回改用 `?`，均保持原有条件与返回语义。包含业务依赖的 `cargo clippy -p rotor-desktop -p rotor-ui --all-targets -- -D warnings` 通过，前述依赖 lint 阻断已消除。

## 旧壳实际对照与第二批布局修正

旧壳使用当前保留的 Tauri/Vue 源码重新构建，放在 `target/legacy-ui-baseline`，资源来自仓库 assets，资料目录为 `target/legacy-ui-profile`。使用既有 `ROTOR_SIMULATE_SHORTCUT_CONFLICT=1` 钩子打开设置，取样时提示已消退；没有安装或修改正式用户资料。它证明保留源码的旧 UI 行为，不冒充未经修改的 v2.6.0 正式安装产物。旧设置默认 500×400，原生默认 820×600；当前截图尚不是同尺寸最终对照。

| 旧壳实际截图 | 对照结果 |
|---|---|
| [通用](ui-windows/legacy-general-zh-dark-source.png) | 紧凑标签/控件行；原生快捷键改为同一卡片中的紧凑行 |
| [概览](ui-windows/legacy-overview-zh-dark-source.png) | 有内存、索引、权限和磁盘详情；原生概览的索引详情覆盖仍需补齐 |
| [快捷操作](ui-windows/legacy-quick-zh-dark-source.png) | 紧凑列表、按需编辑；原生改为常态摘要与展开编辑 |
| [翻译设置](ui-windows/legacy-translator-settings-zh-dark-source.png) | 只显示所选引擎的参数；原生已按引擎筛选，保存只提交当前可见字段 |
| [贴图设置](ui-windows/legacy-pin-settings-zh-dark-source.png) | 路径/缩放与快捷键入口清楚；原生贴图工具改为底部居中主栏，标注时才展开画布工具 |
| [搜索空输入](ui-windows/legacy-search-empty-source.png) | 紧凑搜索输入，无结果内容时不占大面积 |
| [翻译失败](ui-windows/legacy-translation-rate-limit-source.png) | 固定样例 `Rotor makes everyday tasks easier.` 返回 Google 429；保存错误反馈证据，未反复请求端点 |

原生翻译按实际文字换行测量输入/结果高度，空输入不展示空结果大面板，长结果限制窗口高度后独立滚动；输入、取消和后台结果都会刷新尺寸。窗口仍保留创建时的屏幕内放置策略，实际屏幕边缘/IME 场景尚待验证。

[原生紧凑快捷键](ui-windows/native-shortcuts-zh-light-dense.png) 的 820×600 客户区实际可见全部八项与固定保存区；[原生快捷操作摘要](ui-windows/native-quick-collapsed-fixture.png) 已实际呈现。命令是禁用的合成示例，没有执行。

新增 [本地翻译响应夹具](../fixtures/translation_server.py)，只监听 `127.0.0.1:18765`，提供普通/long/slow/error 响应；健康检查通过。它供真实 UI 的传输、长文、取消与失败验收使用，不能代替 Google/DeepSeek 服务验收。新隔离 profile 为 `target/ui-acceptance-fixture`，输入图片是既有合成 640×240 标定图；本轮尚未观察到其贴图恢复窗口，原因未确定。

`cargo test --workspace --exclude rotor --locked` 全部通过；第二批 UI 改动经 native build、native check 和包含依赖的严格 clippy 检查通过。最后新增的贴图恢复诊断计数和后台日志逐条 flush 仅经 clippy 类型检查，尚未重启验证；日志不包含图片内容或配置值。用户随后按物理 Escape 停止 Computer Use，本轮终止桌面输入；其余 UI/Windows 验收继续未关闭。
