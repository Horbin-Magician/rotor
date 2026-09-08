# Windows UI 续作核验（2026-09-08）

起点 `81ec625`；macOS 按用户要求暂缓。上轮已有 UI 改动与截图见 [UI 现代化记录](ui-modernization.md)。

## 合成贴图恢复：夹具坐标修正

上轮的 `target/ui-acceptance-fixture` 写入了 `rect = [300, 300, 640, 240]`、`image_rect = [0, 0, 640, 240]`。`image_rect` 实际是 PNG 在原显示器图像中的范围，不是 PNG 内部裁剪范围；因此得到的 PNG 内部裁剪从 (300, 300) 开始，超出 640×240 源图。

新增只读 `inspect_pins` 示例，调用原生实际使用的 `PinStore::load_from/load_pins/source_crop`。它拒绝不存在的记录，并在任何贴图无效时返回非零，不会把无效或不存在的资料目录记为通过。

```powershell
cargo run -p rotor-screenshot --example inspect_pins -- target/ui-acceptance-fixture
# 实测退出 1：Valid pins: 0; warnings: 1
# Pin 1: Pin crop is outside its source PNG

python doc/gpui-migration/fixtures/prepare_ui_profile.py experiments/gpui-probe/artifacts/fixture.png target/ui-acceptance-fixture-v2
target/debug/examples/inspect_pins.exe target/ui-acceptance-fixture-v2
# 实测退出 0：Valid pins: 1; warnings: 0
# Pin 1: PNG 640x240, crop (0, 0, 640, 240), zoom 100%
```

新生成器令 rect 和 image_rect 的显示器坐标原点一致，拒绝覆盖已有目标目录；图片字节从既有合成 PNG 复制。只读检查前后 `shotter/record.toml` 的 SHA-256 相同。当前受管环境的 PATH 没有 `python`，实测使用工作区运行时提供的 Python 绝对路径执行上述脚本。

此结果说明之前的夹具无效，不能据那次现象认定原生恢复代码存在故障。修正后的实际窗口显示、拖动、导出及 OCR 仍未验收；本轮尚未恢复 Computer Use。

## 概览内容与异步反馈补齐

- 概览和搜索设置复用同一索引面板，展示当前状态、条目数、已索引/总磁盘数、索引文件大小、最近时间及各磁盘的数量/大小/时间，字段来自实际 `SearchIndexStatus`，没有用固定数字替代后台数据。
- 设置视图持有索引请求 ID，忽略其他/过期请求的结果；初始化和刷新概览会请求索引状态，读取中禁用重复刷新。原壳额外的无归属请求已移除。缺失时间显示“暂无记录”，超出时间类型范围也走缺省显示。
- 辅助文字固定为 12px，页面标题固定为 24px，避免 14px 基础字体令这些层级按 rem 继续缩小。
- 贴图元数据和字体加载警告此前只写进壳状态；若设置已打开，用户必须重新打开才看到。现在同时更新已打开视图，保留下一次打开时的提示，并且不激活窗口或抢占焦点。
- `cargo check -p rotor-desktop --offline`、13 项 `cargo test -p rotor-ui --lib --offline`、包含依赖的 desktop/ui `--all-targets --offline -- -D warnings` clippy 通过；警告反馈最后改动后再次执行 desktop clippy 通过。检查未代替 GUI 显示/焦点验收。

合成标定图已从 P0 的既有输出保留到 [calibration.png](../fixtures/calibration.png)，SHA-256 `FCB86244BAD672F03D76442EDD4B61FE041A26FE4C965D2CFE8EDD9CA5A60E52`。后续新 checkout 可用该图运行 profile 生成器，无需依赖被 git 忽略的实验输出。

## 获准恢复桌面验收后的结果

用户明确回复“现在可以继续窗口验收”后恢复桌面检查。沙箱实例未被窗口工具列出；只读确认该实例身份后停止它，在正常桌面以相同 `--no-elevate --no-index` 和 v2 隔离 profile 重启。没有修改正常安装、真实用户资料、自启动或显示设置。

- 日志实际记录 `Restoring 1 pins; 0 metadata warnings`、`Prepared 1 restored pins across 2 displays`，记录中的显示器 ID 更新为当前 ID。用户确认能看到合成标定贴图，完成单张夹具“恢复可见”的人工观察；裁剪、标注、导出和 OCR 尚待验收。
- 给搜索、翻译、贴图和遮罩补了原生窗口名称，贴图名称附记录 ID，设置语言改变时同步相应名称。名称不作为画布元素，不进入导出。窗口工具仍将原生浮窗归到设置页的附属截图，不能据添加名称声称自动操作问题已解决。
- 原生输入翻译热键确实打开了紧凑浮窗，[200% DPI 空输入截图](ui-windows/native-translation-empty-200dpi.png) 已保存。自动点击浮窗会先激活设置，继而触发翻译正常的失焦关闭；已停止重复点击，转请用户输入固定 `short` 样例，当前等待结果确认。
- 本地夹具独立 HTTP 检查：short 返回 200 和 15 字符，long 返回 200 和 729 字符，error 返回 503。这里只证明夹具响应，不证明应用已显示或复制这些结果。
- 新增无窗口 `display_info` 示例，使用同一 GPUI 平台初始化和真实 monitor 配置，只读取拓扑，不创建 UI、不捕获桌面、不访问资料或剪贴板。实测退出 0，输出见 [显示器 TSV](windows-displays-2026-09-08.tsv)：两屏均为 200%，包括竖屏和负 Y 坐标。不同 DPI 混合、热插拔和旋转的行为未由该静态查询覆盖。
- [环境和二进制哈希](windows-ui-environment-2026-09-08.json) 已采集。当前是 Windows 11 build 26200，环境含 Intel Graphics 和虚拟显示适配器；本批为 debug UI 样本，不能用于最低 OS、物理双屏热插拔或 release 性能的结论。
- 该批代码通过 native check、debug build 和包含依赖的严格 clippy。`cargo build -p rotor-desktop --release --offline` 已通过；release `--build-info` 返回 2.6.0 开发身份，`--check-resources --resource-dir src-tauri/assets` 通过。资源检查显式使用开发资源路径，不是已安装包的资源发现或安装验收。

收尾时 `cargo fmt --all -- --check` 发现 6 个历史文件的模块/导入排序及换行差异；运行 rustfmt 后全 workspace 格式检查通过，没有手工改变这些文件的逻辑。格式调整发生在上述 release 构建之后，最终候选仍应从冻结后的源码重新生成构建收据。

最后一次只读窗口观察返回用户按物理 Escape 停止 Computer Use，本轮停止桌面输入并关闭本地响应夹具。`short` 的实际结果未获确认，不据夹具 HTTP 成功认定翻译 UI 已通过；此前用户确认的单张贴图恢复可见结果保留。

## 后续无窗口核对：搜索索引反馈

旧 `SearchInput.vue` 会区分 building/loading/unbuilt/released/error/unavailable，而原生搜索此前仅区分查询加载和空结果。本批在 SearchView 接入初始索引快照及实时 IndexState，显示构建中、加载中、未构建、已释放、未启用和失败；保留查询结果、键盘导航及虚拟列表逻辑。初始查询按请求 ID 接收，已收到实时状态后不再用迟到的初始快照回退状态。

native check、UI `--all-targets --offline -- -D warnings` clippy 通过。全 workspace Windows `cargo check --workspace --offline` 通过，旧 Tauri 壳仍有三个既有 unused/dead_code warning。本批未进行桌面操作或视觉验收。
