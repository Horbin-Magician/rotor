# Rotor 项目优化计划

> 制定日期：2026-07-02
> 范围：Rust 后端性能、前端性能与包体积、代码质量与重构（CI/安全加固不在本次范围）

## 背景

Rotor 是基于 Tauri 2（Rust 后端 + Vue 3/TS 前端）的桌面工具箱，提供文件搜索、截图/贴图（Pin）、OCR 和快捷操作。经过对后端 crates（rotor-screenshot / rotor-searcher / rotor-runtime / rotor-common）、前端（4 个窗口页面）和构建配置的全面探索与逐项核实，确定了一批真实存在的性能与代码质量问题。

**核心发现**（按影响排序）：

1. 全局 `Application` 互斥锁在多个路径上跨阻塞操作持有（同步截取所有显示器、TOML 落盘、PNG 磁盘解码），导致截图/贴图期间所有命令和 WebSocket 数据服务停顿。
2. `img2text` 每次 OCR 调用都重建 OAROCR 管线（从磁盘加载两个 ONNX 模型）——缓存管线是最大的 OCR 性能收益。
3. Pin 窗口每次滚轮缩放都发起一次 `getConfig` IPC 往返；挂载时 4 次串行 `getConfig`。
4. `Cargo.toml` 无 `[profile.release]`（无 LTO/strip）。
5. Pin.vue 1322 行，混杂拖拽/缩放/绘图/OCR/快捷键/菜单等职责。

**已排除的误报**（初步探索报告但核实后不成立）：形态学操作缓冲区克隆（已用 `mem::swap` 双缓冲）、无界线程生成（受显示器/卷数量约束且有 join）、capture cache 无淘汰（每会话整体替换且有 clear）、`get_pin_record` 克隆（约 60 字节小结构体）、大部分 invoke 错误处理缺失（实际已有 catch）、OCR 超时（`spawn_blocking` 无法取消，超时只会孤儿化线程）。

---

## Phase 1 — 快速收益（低风险，先做）

### 1.1 添加 release 编译优化

- `src-tauri/Cargo.toml`（workspace 根）添加：
  ```toml
  [profile.release]
  lto = "thin"
  codegen-units = 1
  strip = "symbols"
  ```
- **不加** `panic = "abort"`：代码中 `collect_captures` 等依赖 `JoinHandle::join` 捕获 worker panic。

### 1.2 缓存 OCR 管线

- `src-tauri/crates/rotor-screenshot/src/img_util.rs:360-383`
- 将 `OAROCRBuilder::new(...).build()` 的结果放入 `static OnceLock<Mutex<OAROCR>>`（模型路径在应用运行期恒定）。第二次及以后的 OCR 调用复用管线。
- 实现时确认 `OAROCR: Send`（`screen_shotter_cmd.rs:356` 的 `spawn_blocking` 需要）；若不满足，改用单一 OCR 工作线程 + mpsc 通道。

### 1.3 Pin 窗口配置缓存

- `src/pages/Pin.vue:286-302, ~959`
- `loadShortcuts` 的 4 次串行 `getConfig` 改为一次 `getAllConfig()`（已存在于 `src/shared/api/core.ts`，Setting.vue 已在用），并同时读出 `zoom_delta` 缓存到模块级 ref。
- 从滚轮处理函数中删除 `await getConfig('zoom_delta')`（当前每次缩放一个 IPC 往返）。

### 1.4 macOS 目录索引改用 FxHash

- `src-tauri/crates/rotor-searcher/src/file_data/volume/default_file_map.rs:33`
- `lookup` HashMap 指定 `fxhash::FxHasher`（依赖已存在，`ntfs_file_map.rs:30` 已有同样用法）。索引构建热路径为 `intern_child`/`find_child`。
- **补测试**：`DirectoryTree` intern/find 往返的 `#[test]`（纯逻辑、无 I/O）。

### 1.5 代码质量微修复（合并为一个 commit）

- `rotor-common/src/config.rs:137-146`：`get_all` 简化为 `DEFAULT_CONFIG.clone()` + `extend`。**补测试**：用户值覆盖默认值、默认值填补空缺。
- `Pin.vue:1183`：contextmenu 监听器存引用并在 `onBeforeUnmount` 移除（当前泄漏）。
- `Pin.vue:888-917`：`saveImage`/`copyImage`/`writeImage` promise 链补 `.catch`。
- `PinCanvas.vue:261-322`：`drawingHistory` 停止 push 无用的 `.clone()`（纯内存浪费）；`backImgLayer.listening(false)`。

## Phase 2 — 后端锁范围收缩（价值最高，中等风险；每项单独 commit，按 2.1→2.2→2.3 顺序）

### 2.1 截屏移出全局锁

- `rotor-runtime/src/application.rs:28-53`、`rotor-screenshot/src/lib.rs:108-121`
- 将 `ScreenShotter::run()` 拆为：
  1. 锁内 `prepare_screenshot_session`：重建 mask 窗口、推进 session、`capture_cache.clear()`、克隆 `CaptureCache` 句柄（内部为 `Arc<Mutex>`，可 Clone）和 `AppHandle`。
  2. 锁外：`capture_all(Monitor::all()?)` → `cache.replace_all(captures)` → `emit("show-mask", session_id)`（保持 replace_all 在 emit 之前）。
- 现有保护机制可复用：Mask 前端的 `is_screenshot_session_current` 会话守卫 + `data_server` 400ms 重试 + 500ms 快捷键防抖。验证方式：连击快捷键。

### 2.2 ShotterRecord 持久化移出锁

- `rotor-screenshot/src/shotter_record.rs:134-151`（调用方 `screen_shotter_cmd.rs:221-258`，每次贴图移动/缩放/失焦触发）
- 锁内序列化为 String（廉价），`fs::write` 交给后台写线程；用 `Arc<AtomicU64>` 保存 save generation，旧代际写入被新代际取代（去抖）。同文件 `save_record_img`（line 74）已有后台 I/O 先例。
- 崩溃可能丢最后一次贴图位置写入——UI 状态可接受，commit 中注明。
- **补测试**：generation 取代逻辑（纯逻辑部分）的 `#[test]`。

### 2.3 贴图图像加载移出锁

- `rotor-runtime/src/data_server.rs:82-87`、`rotor-screenshot/src/lib.rs:220-231`
- `get_pin_img` 拆为锁内阶段（取 `ShotterConfig` 克隆 + `CaptureCache` 克隆）与锁外阶段（磁盘 PNG 解码/缓存回退），解码包在 `tokio::task::spawn_blocking` 中（当前在 async runtime 上跑阻塞解码，属于额外发现的 bug）。
- `get_pin_img` 内的 `capture_cache.clear()` 行为本次**只搬迁不改动**，标记为后续跟进项。

### 2.4（可选，2.1-2.3 顺利后再做）事件驱动的图像就绪通知

- 用 `tokio::sync::Notify`/`watch` 替换 `data_server.rs:89-104` 的 `retry_image` 轮询（20ms×20）。2.1 落地后轮询窗口已大幅缩短，时间紧可跳过。

## Phase 3 — 前端性能与包体积

### 3.1 Vite 分包 + 包体积审计

- `vite.config.ts` 添加 `build.rollupOptions.output.manualChunks`：`vendor-vue` (vue/vue-router/vue-i18n)、`vendor-naive` (naive-ui)、`vendor-konva` (konva)。
- 收益点：Mask/Pin 覆盖窗口在截图时才生成（启动延迟用户可感知），共享缓存的 vendor chunk，避免每个窗口重复解析 App.vue 引入的 naive-ui。
- 改动前后各跑一次 `yarn build` 记录 chunk 尺寸；若 konva 已被隔离在 Pin 路由 chunk 中，删掉该条目。

### 3.2 Mask 放大镜节流

- `src/pages/Mask.vue:304-306, 247-264`
- `handleMouseMove` 经 `requestAnimationFrame` 合并（存最新坐标，每帧只处理一次）。开销大头是每次移动的 `getImageData` 回读。

### 3.3 搜索索引状态改推送（替代 1s 轮询）

- 后端：`rotor-searcher/src/file_data/mod.rs`（`set_state`, line 265）加可选 `state_change_callback`；`rotor-runtime/src/application.rs:107` 仿照现有 `update_search_result` 回调模式接到 `emit_to("searcher", "index-state-changed", ...)`。
- 前端：`src/pages/Searcher.vue:119-152` 监听事件，保留窗口聚焦时的一次初始拉取作兜底，删掉 setInterval。
- 注：现有轮询已在状态 settle 后停止且仅聚焦时运行，此项为中等收益，Phase 3 中最大的一项。

### 3.4 主题/设置响应式小优化

- `src/composables/useTheme.ts` + `App.vue:18-25`：`getColor` 在 computed 内部再建 computed，改为暴露单个颜色 map computed。
- `GeneralSettings.vue:84-96`：`watch(locale)` 重建选项数组改为 `computed`。

## Phase 4 — Pin.vue 拆分（风险最高，最后做；每步单独 commit + 完整贴图冒烟）

目标：1322 行 → ≤500 行，拆到 `src/features/screenshot/composables/`。按风险递增顺序：

1. **纯函数**先抽到 `src/features/screenshot/pinGeometry.ts`：`clamp`、`getResizeCursor`、`getResizedCrop`、`getLogicalSizeForCrop`、`getWindowPositionForCrop`、`parseShortcutKey`（约 lines 304-317, 438-560）。零行为变更，靠 TS 编译器兜底。
2. `usePinSelectionResize`：缩放状态机含 rAF 队列（lines 227-232, 562-694）。
3. `usePinDragSnap`：拖拽/吸附/边缘辉光（lines 219-225, 727-806）。
4. `usePinShortcutsAndMenu`：快捷键配置、keyup、右键菜单（含 1.5 的监听器清理）（lines 256-262, 853-873, 1154-1190）。
5. WebSocket/图像加载与 OCR 留在 Pin.vue（它们是编排层）。

- 风险点：resize 与 drag 共享可变状态（`isDragging`、`isResizingSelection`、`pendingDragViewportUpdate`）——显式传共享 ref，不要复制。

---

## 测试（最小化，仅覆盖被改逻辑）

- Rust `#[test]`：config 合并（1.5）、DirectoryTree intern/find（1.4）、save generation 取代逻辑（2.2）。
- 前端不新增测试框架（无现成 runner，本次不搭建）。

## 验证（每个 Phase 完成后执行）

1. **Rust**：`cd src-tauri && cargo build && cargo test`（workspace 覆盖全部 crate）。1.1 后跑一次 `cargo build --release` 确认 LTO 可编译。
2. **前端**：`yarn typecheck && yarn lint && yarn build`（3.1 前后对比 chunk 尺寸并记录）。
3. **手工冒烟**（`yarn tauri dev`）：
   - **截图**：快捷键 → 全部显示器出现遮罩、放大镜跟随 + 取色、自动矩形高亮、框选生成贴图、Esc 取消；**连击快捷键**验证防抖/会话守卫（2.1 后关键）。
   - **贴图**：拖拽 + 边缘吸附、滚轮缩放（1.3 后仍遵循 zoom_delta）、边角缩放、画笔/矩形/箭头/文字 + 撤销、**连续两次 OCR**（1.2 后第二次应近乎瞬时）、复制、保存、关闭；重启应用后贴图恢复位置/缩放（验证 2.2 持久化）。
   - **搜索**：快捷键唤起、输入即出结果、方向键 + Enter 打开文件、索引状态徽标正常收敛（验证 3.3 与 1.4）。
   - **设置**：改主题/语言/快捷键，重启后仍生效（验证 1.5 config 合并）。

## 依赖与顺序

- Phase 1 各项相互独立，可任意顺序落地。
- Phase 2 按 2.1 → 2.2 → 2.3（逐步收缩锁范围；2.3 复用 2.1 引入的 CaptureCache 克隆模式）。
- Phase 3 与 Phase 2 独立，但 3.3 触碰 `application.rs`，建议在 Phase 2 之后落地避免冲突。
- Phase 4 最后，每个 composable 一个 commit。

## 关键文件

- `src-tauri/crates/rotor-screenshot/src/lib.rs`
- `src-tauri/crates/rotor-runtime/src/application.rs`
- `src-tauri/crates/rotor-screenshot/src/shotter_record.rs`
- `src-tauri/crates/rotor-screenshot/src/img_util.rs`
- `src-tauri/crates/rotor-runtime/src/data_server.rs`
- `src/pages/Pin.vue`
- `src/pages/Mask.vue`
- `src/pages/Searcher.vue`
- `vite.config.ts`
- `src-tauri/Cargo.toml`
