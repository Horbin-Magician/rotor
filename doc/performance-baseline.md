# 性能基线采集

基线按平台、构建身份、硬件、显示配置和场景分别保存。当前提供采集工具和应用侧观测点；工具通过测试不等于 Windows/macOS 全场景基线已经完成。

## 构建与证据

从仓库根目录使用固定工具链，构建开发身份 release 二进制：

```text
cargo build -p rotor-desktop --release --locked --bin rotor-desktop --example capture_pipeline_bench
python scripts/test-performance-baseline.py
```

采集器仅使用 Python 3.10+ 标准库，支持 Windows 和 macOS。Windows 使用 Win32 进程计数器；macOS 使用系统 `ps`，只记录 RSS 和 CPU 时间，私有内存记为 `null`。GPU 内存和物理屏幕可见时间需外部工具另行采集，不能用 RSS、窗口可见标志或 CPU 绘制时间代替。

每次 `record` 必须使用全新的 `target` 子目录。工具保留独立 profile、原始 `samples.jsonl`、应用日志、stdout/stderr、元数据和 `baseline.json`，不自动删除目录。报告包含二进制 SHA-256、版本/身份、Git 提交和工作区状态、采集器哈希、命令、系统/CPU、操作描述和显示配置。工具还保存采集时的 `source.diff` 与 `recorder.py`；它们不证明二进制来自该时刻的源码。比较构建时应在同一源码状态构建并采集，另行保存参与构建的未跟踪源码。

记录准确的 CPU 型号、物理内存、磁盘类型、索引文件数/大小和缓存状态到 `--workload`，显示器分辨率/排列/缩放到 `--display-config`。未记录的字段应明确写“未测”，不能作为完整基线。

显示配置可在采样前用 `cargo run -p rotor-desktop --example display_info --locked` 获取；它复用原生显示枚举，不截图或创建 profile。采样期间不要并行构建或运行基准，以免引入额外负载。

## 空闲与交互资源

Windows 示例（macOS 将二进制路径换为 `target/release/rotor-desktop`，可用 `python3`）：

```text
python scripts/performance-baseline.py record --executable target/release/rotor-desktop.exe --output target/perf-idle-001 --scenario idle --display-config "填写分辨率和缩放" --workload "填写硬件；空合成 profile；索引和快捷键关闭"
```

默认先稳定 60 秒，再采样 120 秒，每 100 ms 一次；稳定期原始样本也保留。统计使用采样期的最近秩 P50/P95/max，CPU 为一个逻辑核心的百分比。短暂峰值可能落在采样间隔之外；Windows 另保存进程生命周期工作集峰值，不能当成该采样期的私有内存峰值。采集结束仅终止本次启动的进程，不代表正常退出验收。

| `--scenario` | 索引 | 快捷键 | 操作 |
| --- | --- | --- | --- |
| `idle` | 关 | 关 | 无操作，用作基础对照 |
| `idle-hotkeys` | 关 | 开 | 无操作，检查日志是否存在注册冲突 |
| `idle-index` | 开 | 关 | 等待索引 Ready 后再判定空闲，未 Ready 的记录不可作为索引空闲基线 |
| `search` | 开 | 开 | 操作者进行合成查询，记录每次输入与缓存状态 |
| `capture` | 关 | 开 | 在合成桌面进行截图、保存/OCR、关闭，再保持空闲 |

索引场景必须在专用合成系统/测试账号中加 `--synthetic-machine` 执行。Windows 索引会枚举 NTFS 卷，macOS 会扫描 HOME 和应用目录；`--data-dir` 只隔离 profile，不隔离搜索范围。不能在真实用户系统上用这个参数绕过隔离要求。macOS 保持真实测试账号 HOME，不改写 HOME 环境变量。

交互采样不会自动发快捷键或截图。测试桌面只显示合成内容；开发快捷键以当前设置为准。测连续截图时将 `--seconds` 增大至足够覆盖“30 次单次操作、连续十次截图→标注/裁剪→保存/OCR→关闭、最后 30 秒空闲”，将各阶段的采集器相对秒数写进操作记录。分别测 1080p、4K、双 4K/混合缩放；不要跨配置合并分位数。截图像素、OCR 数据不得来自真实用户内容。

## 搜索和截图计时

`ROTOR_PERF_TIMING=1` 开启三个日志目标；采集器自动设置。原有 `ROTOR_CAPTURE_TIMING=1` 仍可使用。日志不写查询文本和文件名，只写请求 ID、阶段、显示器 ID 和耗时。

| 观测点 | 含义与限制 |
| --- | --- |
| `startup.profile_initialized` | `run()` 入口到 profile 初始化完成；不含 OS 创建进程之前的时间 |
| `startup.index_ready` | 桌面收到索引 Ready；与 Partial/Error 分开，汇总只取本进程第一次 Ready |
| `search.query_submitted` | 计时从调用搜索服务前开始，仅记录已接受的非空新查询；每次请求独立 ID，追加分页不算首批样本 |
| `search.results_received` | 当前请求结果被 UI 接受；过期结果不会记录 |
| `search.results_painted` | 当前首批结果（也可能为空）的 GPUI CPU paint 阶段，一次请求只记一次；不含快捷键→窗口/输入延迟，不证明 GPU 提交完成 |
| `capture.mask_paint_returned` | 隐藏窗口同步绘制返回；不能视为用户已经看见 |
| `capture.mask_native_visible` | 隐藏绘制后，Win32/AppKit 的原生可见标志为真；不证明 compositor 或屏幕 scanout 完成 |

截图汇总要求同一请求的所有预期显示器都有可见观测，使用最慢显示器的耗时。搜索与截图分别保留已接受请求的开始数、完成数、未完成数和成功样本；取消、失败、观察超时均可能导致未完成，不将它们硬算成成功，也不把所有未完成都标为应用错误。服务拒绝提交的操作不在请求样本内，需在人工操作记录中另计失败数。日志丢失会令 `timing_data_valid=false`；零样本显示为未测，分位数为 `null`。

合并同一场景的多个进程日志：

```text
python scripts/performance-baseline.py summarize target/perf-search-001/profile/rotor.log target/perf-search-002/profile/rotor.log --output target/search-summary.json
```

进程之间必须使用不同 profile 目录，以避免重启后请求 ID 冲突。同一 profile 的 `rotor.previous.log` 和 `rotor.log` 视为一个进程的轮转日志，应一起传入。保留单次采集文件，汇总不替代原始证据。

## 场景与完成条件

1. 在专用合成系统中，用 `fixture --output target/search-small --files 1000` 创建小档目录树；中、大档分别用新目录和 `10000`、`100000`。文件名固定为 `rotor-fixture-*`，两层目录，固定内容。记录系统背景文件规模与排除规则；fixture 文件数不等于整个索引规模。
2. 每平台、每档规模采集至少 30 次新进程搜索和 30 次已加载索引的查询。新进程、新 profile、已持久化索引、OS 冷盘是不同条件，分开标注。当前采集器每次创建新 profile；持久化索引重启场景需独立人工流程，并按进程保存日志。测 OS 创建进程→首批结果物理可见、快捷键→首批结果物理可见时另配外部录像/平台跟踪；当前脚本不自动给出这些指标。
3. 截图各显示配置至少 30 次，另记录上述连续操作的峰值与恢复空闲占用。报告同时列出取消数、失败数、日志缺失、GPU/物理可见指标是否采集。
4. 对优化前后使用同机、同配置、同场景，交替运行顺序。少于 30 次、未记录硬件/显示配置、日志丢失或缺少外部可见性证据时，只能报告有限测量，不能关闭完整基线待办。

## 合成像素准备基准

```text
target/release/examples/capture_pipeline_bench.exe --output target/capture-pixels-001.json
```

macOS 去掉 `.exe`。每档 2 次预热、30 个有效样本，交替测试历史双缓冲路径与当前单缓冲路径。报告保存所有样本、P50/P95 和理论长期持有像素字节；不是实际进程峰值。双 4K 使用等像素数的单缓冲（7680×2160），不代表两个原生窗口。此基准不覆盖原生采集、GPU、编码保存、OCR 或端到端延迟。

完整两平台基线的当前执行状态见 [基线记录](performance-baseline-results.md)。
