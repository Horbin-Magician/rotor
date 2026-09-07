# 迁移实施进度

当前分支：`refactor/gpui`。按 2026-09-07 用户指令持续推进实现并分步提交；尚缺的跨平台实机证据不阻断可逆开发，但不据此标记 G0–G9 验收通过或发布正式更新。

## 已提交

- `3b5be7a`：统一有界后台服务和 GPUI 事件桥，49 项核心/画布测试通过。

- `8c75c47`：资源定位与首次使用后数据路径冻结。

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

## P1 后台服务与原生事件桥

- Services 提供有界事件总线、类型化搜索/翻译/截图/OCR 结果、请求身份、取消及后台并发限制。业务对象不持有 GPUI/Tauri 窗口。
- 配置使用串行队列，磁盘提交后发布快照；UI 读取已提交快照，不等待配置磁盘写锁。退出停止接收请求，关闭发布通道并限时排空已接受的配置写入。
- 旧引擎入口保留，新引擎可注入独立配置；错误对原始和 URL 编码的凭据脱敏。
- 原生入口接 Services，弱引用视图接收事件，主题按钮通过后台队列保存并更新主题；--no-index 用于不扫描磁盘的 UI 验证，release 开发运行可用 ROTOR_RESOURCE_DIR 指定源资源。
- 验证：49 项共享核心/画布单测通过，包含本地 HTTP 翻译、请求 ID、队列容量、关闭时配置顺序落盘；新入口 release、旧入口 check 及新代码 clippy --no-deps -D warnings 通过。
- clippy 明确只约束选定 crate，避免将既有 ntfs_file_map 的历史 question_mark 风格提示混入这一步；历史 lint 留待收敛阶段。
- 用户解锁后，本轮桌面自动化被物理 Escape 停止；不继续桌面操作，原生异步主题交互的实机验收仍待补齐。代码和命令行验证继续。

下一步 P2：单实例激活、托盘/全局热键、窗口注册表与退出流程，然后补齐各功能的原生视图。

## P2 单实例基础

- 使用 Rust 标准文件锁按数据目录区分实例；再次启动只向 loopback 激活端点发送带随机令牌的激活请求，不暴露任意命令或数据接口。
- 元数据大小、地址范围、令牌长度和 socket 等待有边界；退出唤醒阻塞 accept 并回收线程，锁文件不删除，避免 inode 替换导致双主。
- 配置及激活元数据使用私有原子写入；Unix 为 owner-only，Windows 继承数据目录访问控制。
- Windows 的两个单实例单测通过：转发/重新获取、拒绝错误令牌、空闲客户端不阻塞退出；common 既有测试通过。尚未以桌面自动化执行多进程窗口激活。
