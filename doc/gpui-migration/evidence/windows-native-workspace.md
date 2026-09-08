# 原生 workspace、CI 与无 Node 构建（2026-09-08）

根 workspace 已移除旧 `rotor` Tauri 包，只包含十个原生/共享 crate 和 xtask。Cargo.lock 不再包含 Tauri、Wry、WebView2 包。版本读取与变更只使用根 Cargo.toml/Cargo.lock；生产身份测试改为冻结的 v2.6.0 名称、标识、资料目录与程序名契约，不再读取旧壳配置。

## 构建与文档

- 原生 CI 移除 legacy Node/Yarn 任务，默认 Windows；macOS 仅在明确选择时加入。
- 发布 workflow 改为复用原生候选构建，先核对 tag、版本和 checkout，再构建/签名生产候选并创建未发布草稿；更新源切换仍单独执行。
- 新增附件整理器，核对完整文件集合、哈希、生产身份、版本、签名文件是否入清单及重复文件，全部验证后才写草稿附件。签名密码学检查属于上游 Rust 签名步骤，整理器不冒充重新验签。
- 中英文 README、AGENTS 和 native 分发说明切换到原生命令，品牌资源已复制至 doc/branding。
- xtask 构建显式指定自身读取的 target 目录，避免继承 CARGO_TARGET_DIR 后误读取其他目录中的旧程序。

## 实际验证

原生 workspace 的 172 项测试通过（原先 174 项中的两项旧 Tauri 适配测试已退出 workspace），完整 workspace 严格 clippy、格式检查和 Windows 共享核心依赖边界检查通过。生产身份 common 14 项测试通过；版本读取及 set-version dry-run 通过，没有实际变更版本。

单独复制 137 个原生输入文件到 `target/native-only-source-check`，逐字节核对与主工作区一致；副本不含 package.json 或 src-tauri，使用新的 `target/native-no-node-check` 构建目录。在子进程 PATH 中移除全部 Node/Yarn 入口后，离线 workspace check 与完整 debug build 通过。独立程序的 `--build-info` 通过，`--check-resources` 指向副本根 assets，证明未依赖旧 Tauri 生成的相邻资源目录。详见[机器可读记录](windows-native-only-2026-09-08.json)。这不是清空依赖下载缓存后的离线安装，也不是 macOS 或 release 性能证明。

发布附件整理器的 5 项合成测试通过，覆盖双平台独立收据、文件篡改、缺失/未入清单签名、错误身份、重复附件及额外文件。workflow YAML 与内嵌 Python 本地解析通过；未运行远端 CI、使用真实签名密钥、创建远端草稿或发布。

## 旧文件清理边界

用户已确认[91 个旧文件清理清单](legacy-removal-plan.md)，并重新说明完整访问权限。删除命令仍被工具自动审批在执行前拦截，只返回 blocked by policy。因此源码文件实际保留原位；原生构建的解耦已通过独立副本验证，但 P9-03 的物理清理没有完成。不得将此写为已删除，也不得换工具绕过拦截。可视化测试按用户要求跳过，macOS 暂缓。

## 当前提交的 release 与打包

基于 `0dd6792`，再次在无 Node/Yarn PATH 下完成开发身份 release，并设置故意不同的 CARGO_TARGET_DIR；xtask 正确使用自身 target 目录，没有创建该环境变量指向的哨兵目录。正式身份 release 随后通过。

两种身份均从中文/空格暂存目录生成 NSIS 包，通过文件清单、安装器 VERSIONINFO、`--build-info` 和外部工作目录下的资源发现检查。各暂存目录包含 31 个清单文件，运行库仅 DirectML.dll；没有 Tauri/WebView DLL。完整路径、源码收据、哈希见[新候选记录](windows-native-cutover-candidates-2026-09-08.json)。本批没有重复安装、签名或发布；先前静默安装结果仍按其对应提交归属。

删除请求在用户再次说明完整访问后仍遭执行前拒绝，最终只读核对清单中的 91/91 个文件仍存在。原生改造和证据已分阶段提交，不据访问权限声明推断实际删除成功。
