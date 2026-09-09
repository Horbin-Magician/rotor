# P0 实验退役记录

日期：2026-09-09。用户授权清理 experiments 目录。

## 删除范围与恢复

删除 experiments/gpui-probe 的 19 个 Git 跟踪文件（原型源码、锁文件、工具链、合成样例、脚本和打包配方），同时删除 .github/workflows/gpui-p0.yml 和原型构建缓存。根工作区不再保留对应 exclude 项。

清理前源码提交：`97d2b29be389f3e88019a5d09fd975220a03662a`。可只读查看历史 README：

```powershell
git show 97d2b29be389f3e88019a5d09fd975220a03662a:experiments/gpui-probe/README.md
```

需要完整源码时，可从该提交导出 experiments/gpui-probe；不必回退当前工作区。

## 本地证据

原 artifacts 目录和 artifacts-resolve.log 已完整移动到仓库根目录下的
`.gpui-probe-archive-20260909.local/`，由既有 `*.local` 规则忽略，不提交日志、截图或资料内容。
其中 inventory.csv 保存归档文件相对路径及大小，tracked-files.txt 保存删除的源码清单。
此归档仅存在于本机，不由 Git 备份；历史文档中的 experiments/gpui-probe/artifacts
路径按该归档中的 artifacts 路径查找。已提交的迁移证据及 calibration.png 保持原位。

ocr_smoke 示例改用 `target/ocr-smoke` 作为合成 profile 和图片输出目录。
当前入口文档已更新；其余迁移计划、命令和实机记录中的实验路径属于历史上下文，
不再代表当前开发入口。

## 验证

- 通过：`cargo fmt --all -- --check`。
- 通过：`cargo check --workspace --all-targets --locked`，包含 OCR 示例编译。
- 通过：`git diff --check`；确认 crates、.github、Cargo.toml 无实验路径引用，experiments 目录不存在，归档被 Git 忽略。
- 未运行：完整测试套件及安装验收；本次修改仅涉及原型退役、输出路径和文档。
- 按既有用户要求跳过视觉测试，macOS 验证暂缓。
