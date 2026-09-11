# Rotor 3.0.0 更新计划

状态：实施中。2026-09-11。

## 版本定位与范围

3.0.0 是以 Rust + GPUI 为基础的全新原生版本。彻底移除 Tauri 运行时、间接依赖和专为旧客户端保留的适配；不提供 2.x 资料导入、协议兼容、覆盖升级或回退到 2.x 的能力。

本计划优先于 AGENTS.md 中旧版兼容相关要求，实施第一步同步修订这些要求。保留原生应用自身的取消、过期结果拒绝、事务写入、签名验证，以及 3.x 后续更新的失败恢复能力。

新版本使用独立资料命名空间，开发与正式身份继续隔离；不读取、迁移或删除用户原有资料。安装和自启动标识统一按新版本设计，不以兼容旧安装为约束。历史 Git 提交不重写，第三方版权声明不篡改。

## 执行顺序

### 1. 更新项目约束与应用身份

- [x] 修订 AGENTS.md：删除保留旧安装身份、旧快捷键契约、旧记录格式、旧更新密钥命名和旧客户端升级验收的要求。
- [x] 统一 native/app.toml、native_app.rs、file_path.rs、desktop 启动参数、安装器及 macOS bundle 的身份定义，减少重复硬编码。
- [ ] 明确安装目录、卸载项、单实例、自启动及更新通道名称，验证彼此一致。
- [x] 保持 workspace 与锁文件中的应用包版本统一为 3.0.0。

完成标准：3.0.0 全新启动、安装及卸载正常，不需要考虑旧版情况。

### 2. 删除旧客户端兼容代码

- [x] 删除 rotor-platform/src/legacy_instance.rs、模块导出和 desktop 中对应 lease 生命周期。
- [x] 删除首次启动旧资料检测、自动备份迁移及其调用；清理 profile_migration 中专用于旧资料的实现与 xtask 命令。通用备份功能若仍有原生用途则单独保留。
- [x] 删除 native/windows.nsi 的旧 `/UPDATE /ARGS` 握手、旧互斥体等待和 xtask 的 LEGACY_MUTEX 参数；保留原生 `/PARENT` 等实际使用的交接。
- [x] 删除 startup 中针对旧启动项的转换，以及仅为旧安装、可执行文件和 bundle 身份保留的分支。
- [x] 清理截图记录、搜索状态与配置序列化中仅为旧版提供的字段别名、格式转换和兼容类型；先梳理当前 GPUI 调用者，保留实际业务所需的字段和未知字段处理能力。
- [ ] 删除旧资料读取器、旧格式 roundtrip、旧更新清单和公钥夹具；以全新原生配置与贴图持久化测试替代。

完成标准：运行时与打包工具不再包含旧客户端检测、迁移或兼容入口，原生截图、OCR、翻译、搜索和设置行为不回退。

### 3. 清理依赖与更新签名实现

- [ ] 用 cargo tree 定位 tauri-winrt-notification 的完整引入链，替换通知后端、关闭不需要的上游 feature，或调整直接依赖，确保锁文件和目标平台依赖图均无 Tauri 包；不能只手改 Cargo.lock。
- [ ] 搜索并清理有效源码、脚本及工作流里的框架残留、旧注释与无用依赖。
- [ ] 将 TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD 改为 ROTOR_SIGNING_PRIVATE_KEY / ROTOR_SIGNING_PRIVATE_KEY_PASSWORD；移除旧名称回退，列出仓库 Secrets 的配套操作。
- [ ] 以原生签名格式为唯一生成与验证契约；移除旧客户端编码适配、旧签名模式回退及公钥必须与旧版一致的测试。保留现代签名校验、大小限制及篡改拒绝。
- [ ] 统一原生公钥来源；若更换密钥，在发布环境配置新私钥并验证配对，不将私钥写入仓库。

完成标准：Cargo.lock 与依赖图无 Tauri 依赖；代码和 CI 不读取旧 Secret 名称；签名、错误密钥、篡改与截断产物测试通过。

### 4. 完成原生发布与自动更新

- [ ] 为 3.x 定义 stable / preview 通道，统一 native 配置、updater 常量、runtime 选择逻辑及镜像脚本，删除迁移期 gpui-preview-production 等命名。
- [ ] 使用独立原生更新清单地址，不写入旧客户端的 latest.json；无需实施旧客户端推广或升级桥接。
- [ ] 修改 release-manifest，按本次实际发布的平台生成清单；支持 Windows 单平台，缺失所选平台产物或签名必须失败。
- [ ] 将经过验证的清单纳入草稿附件，区分固定版本产物地址与稳定通道清单地址；补齐发布后通道推广、镜像同步和撤回步骤。
- [ ] 发布正文从版本文档读取；删除工作流中的候选版占位文案。
- [ ] native-checks 覆盖 publish.yml 变更及 master push，补充发布包和所选平台清单一致性检查。
- [ ] 3.0.0 默认按 Windows x64、macOS双端首发准备。

完成标准：从 v3.0.0 标签可生成具备安装包、签名、校验记录、发布说明和更新清单的可审阅草稿；全流程不依赖 2.x 产物或协议。

### 5. 更新用户文档与分发资源

- [ ] 修复中英文 README 的 validation-status.md 失效链接，新增真实验收状态记录。
- [ ] 更新版本命令和签名示例为 3.0.0，移除旧版迁移、恢复及兼容指南；修正“内置字体”为系统字体。
- [ ] 增加当前 GPUI 搜索、截图标注、翻译及设置界面的截图，移除预览阶段占位说明。
- [ ] 补齐模型来源及许可资料；将项目与相关第三方许可纳入安装包和产物清单。
- [ ] 清理 diff --check 报告的行尾空白。

完成标准：文档链接有效，命令与实际工作流一致，许可资料随包分发，无未经实测的性能或平台支持承诺。

### 6. 验证、合并与发布

- [ ] cargo fmt --all -- --check。
- [ ] cargo check --workspace --locked。
- [ ] cargo test --workspace --locked。
- [ ] cargo clippy --workspace --all-targets --locked -- -D warnings。
- [ ] 核心依赖约束、发布元数据 Python 测试及新依赖图检查。
- [ ] OCR smoke、全新原生资料 roundtrip、开发与正式身份资源发现检查，均使用合成资料。
- [ ] 通过 xtask build → stage → verify → package 创建新目录中的安装包。
- [ ] 在隔离环境验证全新安装、启动、卸载，以及 3.x 原生更新失败恢复；无 2.x 验收任务。
- [ ] 人工验证多屏与缩放、截图及贴图、OCR、IME/快捷键、翻译取消、自启动和托盘生命周期。
- [ ] 检查提交与主分支差异，完成 PR 验证后合并 master；对最终提交创建 v3.0.0 标签、审阅发布草稿，再公开发布和推广原生通道。

完成标准：代码检查、安装结果与人工验收分别留有记录；所发布平台通过对应验收。公开发布不以未执行的检查作为通过依据。

## 建议提交拆分

1. docs: define the native 3.0.0 release scope
2. refactor(desktop)!: remove legacy client integration
3. refactor(runtime)!: adopt native profile and record contracts
4. build: remove obsolete framework dependencies
5. refactor(updater)!: adopt native signing and release channels
6. ci: complete native release metadata and publishing checks
7. docs: prepare the 3.0.0 release guide

涉及不兼容变更的提交用 BREAKING CHANGE footer 说明不支持旧客户端升级与资料导入。实现中按实际差异调整拆分，不为提交数额外重构。
