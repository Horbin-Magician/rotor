# P0 旧版基线与测量记录

源码：`40addee065765ed343e665d6a712b3e9cbe2bdcc`，Rotor v2.6.0；实施起点 `b084f4d`。原型不引用业务 crate，旧 workspace/锁文件/资源未迁移。

## 已固定的输入

- 已安装程序：`C:/Program Files/Rotor/rotor.exe`，ProductVersion/FileVersion 均为 2.6.0；SHA256 `90340F4F5AD12BDB3EE119E3BB8B0CF1BC576654D7A8CBEBA7E2EC6809488E75`。版本资源不能单独证明它与源码 SHA 逐字节对应。
- 已从真实配置采集语言、主题、缩放、快捷键、翻译引擎/目标语言的脱敏子集，保存在本地 `experiments/gpui-probe/artifacts/old-baseline/config.sanitized.toml`；没有采集密钥、命令或个人路径，没有写回旧数据。
- 用户数据格式：`.rotor/config.toml` 为字符串表；贴图为 `.rotor/shotter/record.toml` 的 `workspaces.default.shotters` 及 `default/{id}.png`。可提交的合成样例见 `experiments/gpui-probe/fixtures`；它们不冒充真实贴图副本。
- 已采集固定 v2.6.0 发布的 [更新元数据](evidence/v2.6.0-latest.json)，SHA256 `F6445CFDF36D20B015074C7724EF0ED407A74521E0B93FFB9814CA986E56D1EB`。安装包本体和签名验证仍待采集/执行。

旧导出入口 `src/pages/Pin.vue` 的 saveImage/copyImage/imgToText 先调用 `syncCurrentScaleFactor()`，再 `stage.toBlob({ pixelRatio })`；Stage 尺寸和 zoom 参与输出，不能承诺恒定源图尺寸。

原型输出固定 640×240，只用于 RGBA/BGRA、alpha、PNG 和中文验证，P6 仍需旧 Konva 导出样例作为兼容依据。

## 初步待机样本

2026-09-07，当前已安装的 v2.6.0 会话，主进程及子进程共 11 个。原型已退出，旧程序没有被本轮测试操作；机器同时有其他桌面应用，索引/贴图/OCR 具体业务状态未冻结。这是探索性记录，不能拿来宣称新旧性能差异或冻结预算。

[原始 CSV](evidence/old-v2.6-idle.csv) · [汇总 JSON](evidence/old-v2.6-idle-summary.json)

| 指标 | 实测 |
|---|---:|
| 采样条数 | 120 |
| 首尾实际时间跨度 | 174.988 秒 |
| 进程树数量 | 11，期间未变 |
| 单逻辑核 CPU 平均 | 15.733% |
| Private Bytes 均值 | 799.811 MiB |
| Private Bytes 最小 / 最大 | 799.684 / 799.957 MiB |
| Working Set 均值 | 748.561 MiB |
| 句柄最小 / 最大 | 5236 / 5240 |

CPU 按累计 CPU 秒差 / 实际时间跨度计算，累计值单调。该次采集的 CIM 查询耗时叠加固定 sleep，120 条记录跨约 175 秒；采集脚本随后改为按 Stopwatch 截止时间和绝对采样时刻调度，汇总始终使用实际时间戳。

这一既有会话样本超过初始待机 CPU 目标，但未确认业务空闲状态，不能据此归因，也未放宽目标。仍需在固定语料、无贴图、OCR 释放且索引稳定的条件下重测。

## 剩余基线

| 项目 | 状态 / 需要的证据 |
|---|---|
| 源码、安装版本、脱敏配置和发布元数据 | 已采集；未将版本号冒充源码一致性证明 |
| 旧功能截图/操作录像 | 未测：搜索、翻译三种引擎、截图/贴图/OCR、设置、快捷动作 |
| 真实贴图副本 | 未采集；目前只有合成 fixture |
| zoom × DPI 导出矩阵 | 未测：同一 640×240 输入，zoom 50/100/200%，DPI 100/150/200%，逐一记录尺寸和像素 |
| 冷启动/热唤起/长驻 | 未测：20 次冷启动、10 次预热+100 次交互、2 小时常驻 |
| 完整性能预算 | 尚未冻结；上述待机数据仅为初步样本 |
| 已知旧问题 | 代码发现配置先改内存再写盘；失败回滚边界需 P1 验证 |

P0-01 只有在上述实测补齐后才能勾选。采样不重启旧版、不修改其配置或删除贴图。
