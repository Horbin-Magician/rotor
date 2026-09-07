# 迁移实施进度

当前分支：`refactor/gpui`。按 2026-09-07 用户指令持续推进实现并分步提交；尚缺的跨平台实机证据不阻断可逆开发，但不据此标记 G0–G9 验收通过或发布正式更新。

## 已提交

- `e0010f9`：类型化搜索请求/索引状态与线程退出，17 项搜索测试通过。

- `1c4a797`：P1 旧壳适配迁出共享核心，30 项核心测试与两端依赖边界检查通过。

- `0c6e182`：P1 根 workspace、正式 native/ui/canvas 入口和双壳 CI。

- `57c1f6a`：P1 配置事务和数据目录隔离。
- `25f2b56`：P0 独立原型、依赖锁定、Windows 实机证据、试验打包和采样工具。跨平台/完整基线缺口见 dependency-baseline.md。

## P1 数据服务

- 新增 `ROTOR_DATA_DIR` 与启动前一次性路径注入；显式空路径不会落回真实用户目录。
- `ConfigService::load_from` 可使用独立数据副本，损坏文件加载失败，不静默覆盖。
- 配置先写同目录临时文件并 flush/rename，成功后才更新内存；多键修改一起提交，未知键保留。
- 旧 `AppConfig` API 继续可用；读取失败的全局实例禁止后续写入，避免以默认值覆盖损坏数据。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml -p rotor-common --offline`，6 项通过（包括写入失败回滚、损坏文件保留、未知键往返和路径隔离）。旧依赖版本没有升级，锁文件只增加已有 tempfile 的测试引用。

下一步：根 workspace、正式 desktop/ui/canvas crate；随后逐项迁出旧壳适配，接入原生窗口和业务服务。P1 完成门槛尚未通过。

## P1 根 workspace

- 根 workspace 包含 10 个成员，default-members 为 rotor-desktop；新增 rotor-ui、rotor-canvas。P0 experiment 保持独立排除。
- 旧 src-tauri/Cargo.lock 迁至根，旧 release profile 原样上移，旧 manifest/前端/Tauri CLI 入口保留。构建产物改为根 target，旧发布 CI 缓存同步更新。
- 从旧锁文件解析，补齐缺失缓存后保留绝大多数旧版本；必要变化集中在 futures 家族（GPUI 要求至少 0.3.32，配套解析 0.3.34）、regex-automata（新 globset 要求 0.4.18）以及新增 Linux 桌面依赖带来的 zbus_names/zvariant 家族。没有进行全量 cargo update。
- 正式原生入口先接真实 ConfigService 读取；默认使用 home/.rotor-gpui，ROTOR_DATA_DIR 可指定隔离副本；--check-config 只输出路径/键数量，不输出凭据。
- 当前原生设置视图只显示基础配置快照，不是 P3 完整设置交付。
- 验证：native check、3 个新增 crate 的 clippy -D warnings、common/canvas 共 7 项测试、旧 rotor check 全部通过。旧入口现有 Windows setup 的 unused app 警告记录保留。
- 本机预装 stable 实际为 rustc 1.97.0，验证使用 cargo +stable；仓库和 CI 固定命名工具链 1.97.0。

开发入口：在根运行 cargo run -p rotor-desktop（原生），旧版仍使用 yarn tauri dev；P0 则在 experiments/gpui-probe 独立运行。不得把基础窗口当成已完成的功能迁移。

## P1 旧壳适配与核心边界

- 旧 application/tray/screenshot_data、搜索/翻译窗口和截图窗口适配集中到 src-tauri/src/integration；IPC 命令名、事件名、payload 和快捷键语义保持。
- 搜索器保留后台索引/查询核心，旧窗口包装委托核心；截图捕获、缓存、记录、OCR 和会话恢复判断仍在截图 crate；翻译引擎独立，模拟复制下沉 rotor-platform。
- rotor-runtime 的快捷动作解析改用与原插件同版本的独立 global-hotkey 类型；不依赖 Tauri 插件。
- 全部共享核心 crate 已移除 Tauri/GPUI/WebView normal/build 依赖，Windows/macOS 两个目标闭包已通过脚本检查，已接入 CI。
- 验证：旧 rotor check 通过；runtime 2、screenshot 7、searcher 12、translator 9，共 30 项核心单测通过。所有测试使用隔离数据目录。
- 正式 native release 构建通过，--check-config 成功读取隔离 ConfigService。窗口创建及可访问性树可见，但桌面已锁定，视觉/交互复测暂未完成；继续命令行与代码实现。

下一步补齐类型化请求、过期结果隔离、后台任务生命周期和 GPUI 事件桥，再进入完整原生桌面服务和功能界面。

## P1 搜索请求身份与退出

- 每次查询生成进程内唯一 QueryId，批次携带原始 ID；相同查询字符串和不同服务实例也不复用身份。
- 索引状态改为类型化 IndexState，序列化保留旧 JSON 的 unbuilt/unavailable 等值；旧适配继续发送原有字符串事件和结果 tuple。
- 关闭服务后拒绝新查询，Drop/显式 shutdown 发送终止消息；修复接收端断开后原事件循环持续空转的问题。
- 验证：搜索器 17 项测试通过，覆盖重复查询身份、实例间 ID、停止后拒绝、断开后线程退出、旧 JSON 名称；旧 Tauri check 通过。

## P1 资源与数据路径稳定性

- 数据目录在首次解析后冻结，阻止配置、索引和截图在运行中因环境变量/工作目录变化而分流到不同目录。
- ResourceLocator 支持 Windows 同目录 assets、macOS Contents/Resources/assets，以及显式 ROTOR_RESOURCE_DIR；开发源资源回退仅用于 debug。
- 资源解析拒绝父目录/绝对路径逃逸和越界符号链接；显式覆盖无效时返回错误，不静默读取其他资源。
- 9 项 common 测试通过，包括目录冻结、两种安装布局和无效覆盖；新旧入口 check 通过。
