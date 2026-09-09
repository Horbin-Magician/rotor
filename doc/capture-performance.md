# 截图响应路径与测量

Windows 的目标是应用常驻后，从截图快捷键回调到包含本次截图的 mask 首帧实际上屏小于 50ms。
当前完成工程优化，尚未完成实际呈现延迟验收；不能用 `ShowWindow` 返回时间代替上屏时间。

## 实现

- 启动时创建每屏隐藏 mask，预热原生窗口和渲染器。取消后保留窗口，清空交互状态、释放本次截图的图像和 atlas 条目。显示器配置变化时重新创建对应窗口。
- 截图使用独立常驻协调线程、每屏常驻 worker 和可复用 DIB/DC；启动预热不读取桌面像素。通用后台额度耗尽不会拒绝截图。
- 协调线程只保留一个待处理请求，新请求替换旧待处理请求。已经进入 OS 的调用无法强制中断，完成后仍按 operation id 淘汰过期结果；每屏队列有界，保留多屏捕获超时。
- Windows 使用 `BitBlt` 写入 top-down DIB，`GdiFlush` 后复制出 BGRA；遮罩直接接收这份像素，不再执行 BGRA→RGBA→BGRA。取色直接读取 BGRA；RGBA 在显示请求完成后由后台检测按需生成并缓存，导出继续使用 RGBA。
- 窗口矩形枚举与多屏取图重叠执行，仍保留取图前后、准备显示前的显示器一致性校验。
- 仅当之前隐藏过截图遮罩、尚未完成后续捕获时等待 `DwmFlush`。隐藏失败的窗口直接销毁，不放回缓存。
- Windows 在没有持有 GPUI App/Window/Entity 借用时，先调用自身窗口的绘制处理，再请求显示，避免依赖隐藏窗口的普通帧调度。此处依赖锁定版本 GPUI 0.3.3 的 `WM_PAINT`→`draw_window`→draw/present 路径；升级 GPUI 时需重新核对。调用限制为当前进程、当前 UI 线程拥有的窗口。
- macOS 不做启动窗口预热，保留关闭时销毁 mask 的行为；macOS 验收仍暂缓。

代价是常驻每屏窗口、渲染器和一份 DIB 缓冲区。待机内存、GPU 资源及连续截图后的资源回落仍需在性能验收中确认。

## 分段日志

默认不记录性能日志。设置 `ROTOR_CAPTURE_TIMING=1` 后，`rotor.log` 记录 `capture_latency`，只有请求 id、阶段、显示器 id 和微秒耗时，没有截图像素或凭据。

```powershell
$env:ROTOR_CAPTURE_TIMING = '1'
cargo run --release -p rotor-desktop --locked -- --no-elevate --data-dir target/capture-timing-synthetic --no-index
```

使用新的隔离配置目录和只含合成内容的桌面环境。当前工作未运行以上交互命令。

快捷键请求的所有 `elapsed_us` 共用回调入口的单调时钟起点；托盘等非快捷键入口从 UI 分发开始计时。

| 阶段 | 含义 |
| --- | --- |
| `capture_requested` | UI 已提交请求，包含快捷键分发时间 |
| `worker_start` | 专用协调线程开始处理 |
| `desktop_settled` | 条件性桌面等待结束，未等待时也记录 |
| `topology_before` | 捕获前显示器信息完成 |
| `window_rectangles` | 并行枚举窗口完成 |
| `pixels_captured` | 所有屏幕像素和窗口矩形均已收集 |
| `capture_complete` | 捕获后拓扑校验完成 |
| `capture_received` | UI 收到本次捕获结果 |
| `images_prepared` | 显示数据与再次拓扑校验完成 |
| `mask_frames_scheduled` | 所有 mask 已更新数据，绘制任务已调度 |
| `mask_paint_returned` | 单个 mask 的绘制处理返回；不能证明物理呈现成功 |
| `mask_show_requested` | 单个 mask 请求显示，含显示器 id |
| `all_masks_show_requested` | 所有 mask 均已请求显示，开始后台矩形检测 |

并行阶段不应简单相加。日志输出异步且有界，负载过高时可能丢弃日志；验收样本必须具有完整阶段。
分别统计首次触发、重复触发、首屏和全屏的 P50/P95/P99；覆盖单屏、多屏、4K、混合 DPI、后台繁忙与连续取消重试。
实际上屏的 50ms 验收还需要呈现追踪或外部观测，以上软件时间戳只能定位延迟来源。

## 验证边界

2026-09-09 Windows 工程检查通过：`cargo fmt --all -- --check`、
`cargo check --workspace --locked`、`cargo test --workspace --locked`、
`cargo clippy --workspace --all-targets --locked -- -D warnings` 和字体/许可证校验。
最后的 UI 输入状态和像素长度校验修正后，另行重跑 rotor-ui、rotor-desktop 测试及全工作区 Clippy，均通过。

自动化回归使用合成像素、内存 DIB、隐藏的原生窗口夹具和临时配置目录。
隐藏窗口夹具只验证同步绘制消息和可见性，不是 GPUI 画面或实际呈现验收。
视觉 UI、真实多屏截图时序及 macOS 测试按现有约定跳过或暂缓，不计为通过。
