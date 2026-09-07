# 迁移实施进度

当前分支：`refactor/gpui`。按 2026-09-07 用户指令持续推进实现并分步提交；尚缺的跨平台实机证据不阻断可逆开发，但不据此标记 G0–G9 验收通过或发布正式更新。

## 已提交

- `25f2b56`：P0 独立原型、依赖锁定、Windows 实机证据、试验打包和采样工具。跨平台/完整基线缺口见 dependency-baseline.md。

## P1 数据服务

- 新增 `ROTOR_DATA_DIR` 与启动前一次性路径注入；显式空路径不会落回真实用户目录。
- `ConfigService::load_from` 可使用独立数据副本，损坏文件加载失败，不静默覆盖。
- 配置先写同目录临时文件并 flush/rename，成功后才更新内存；多键修改一起提交，未知键保留。
- 旧 `AppConfig` API 继续可用；读取失败的全局实例禁止后续写入，避免以默认值覆盖损坏数据。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml -p rotor-common --offline`，6 项通过（包括写入失败回滚、损坏文件保留、未知键往返和路径隔离）。旧依赖版本没有升级，锁文件只增加已有 tempfile 的测试引用。

下一步：根 workspace、正式 desktop/ui/canvas crate；随后逐项迁出旧壳适配，接入原生窗口和业务服务。P1 完成门槛尚未通过。
