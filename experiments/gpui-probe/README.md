# Rotor GPUI P0

独立实验 workspace；不加载 Rotor 配置、不注册旧版快捷键、不执行更新。依赖、平台结果及 G0 缺口见 [dependency-baseline](../../doc/gpui-migration/dependency-baseline.md)。

从本目录执行（从仓库根运行时使用 `cargo +1.97.0` 并指定 `--manifest-path experiments/gpui-probe/Cargo.toml`，确保使用子工程锁定的工具链）：

```text
cargo check --locked
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
```

Windows：`./scripts/run.ps1` 捕获启动日志。macOS：`sh packaging/macos.sh` 生成隔离的未签名 app；从终端直接执行 app 内二进制并重定向 stderr，保留 GPU 启动错误。`LSUIElement` 验证隐藏 Dock，但不代表跨 Spaces/fullscreen 已通过。

默认启动普通窗口、两个透明 PopUp 和每屏一个遮罩。传 `--window-only` 可单独验证最后窗口关闭/热键恢复，默认启动行为不变。先关闭遮罩，测试下层窗口；每个窗口均提供关闭与退出。关闭所有窗口后，进程应常驻；托盘“显示”或 `Ctrl+Alt+Shift+G`（macOS 也是 Control+Option+Shift+G）恢复普通窗口。通过托盘菜单退出。热键冲突/托盘创建失败会终止初始化并写日志。

验证流程：

1. 输入 `中文输入测试 abc`，在候选框尚未确认时切换窗口，测试 Enter/Escape、光标移动、粘贴及撤销，记录实际 IME 名称。
2. 比较红/绿/蓝色块、半透明红色、透明区域及 1px 线；图像以物理像素 1:1 显示。上方图像含 resvg 离屏文字，下方 GPUI 原生文字使用同名字体；两者不同的排版后端是待验证的候选方案，不宣称像素一致。
3. 复制时会从系统剪贴板回读并逐像素校验；再粘贴到画图/预览，检查 640×240、alpha 和色彩。保存到中文/空格路径；取消或写入失败后窗口继续可用。
4. 把窗口移到负坐标/不同 DPI 显示器，记录窗口标题内容的 bounds、scale factor、图片清晰度。重新创建遮罩取得最新显示器快照；暂不自动响应热插拔。
5. 检查各窗口焦点、置顶、关闭恢复、macOS Spaces/fullscreen；PopUp API 不是行为验证证据。

CPU 离屏导出（不需要 GPU）：

```text
cargo run --release --locked -- --export-fixture artifacts/fixture.png
```

父目录必须存在。导出使用 Windows Microsoft YaHei/macOS PingFang SC；缺少比较字体时报错，不能以空白文字通过测试。字体来自操作系统，不复制/分发字体文件。PNG 是原型样例，不是旧 Konva 实测结果。

`fixtures/config.toml` 和 `fixtures/record.toml` 是合成兼容输入，不能覆盖真实 `.rotor`。PNG 可生成后配为 `shotter/default/1.png`；不自动启动旧应用读取该目录。

Windows 环境和性能采集：

```powershell
./scripts/collect-environment.ps1
# 手动启动指定 release 程序；待机稳定 60 秒后执行
./scripts/measure-idle.ps1 -Exe 'C:/path/to/rotor.exe'
./scripts/check-dependencies.ps1
```

待机脚本每秒记录进程树 CPU 累计值、Private Bytes、Working Set 和句柄；CPU 使用累计值差除以实际采样秒数得到单核比例。若进程树成员变化导致累计值下降，该轮 CPU 不可用于比较。另按 validation.md 收集冷启动/热唤起/截图 p50/p95，不能以此脚本代替交互测量。

Windows 最小试验安装器：运行 `./scripts/package-windows.ps1`（自动查找系统或 Tauri 缓存内的 NSIS，也可传 `-Makensis`）；直接调用 NSIS 时必须传绝对路径 `/DPROBE_EXE=... /DOUTPUT_FILE=...`。安装目录与卸载注册键均独立；不接入正式更新通道。macOS bundle 资源由可执行文件内嵌 Kit 资源加 Resources/P0-README.md 组成；OCR 尚未接入，不把旧模型复制成“迁移完成”。
