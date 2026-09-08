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
