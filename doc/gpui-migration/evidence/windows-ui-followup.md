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
