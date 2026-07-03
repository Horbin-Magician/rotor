<p align="center">
  <a href="https://github.com/Horbin-Magician/rotor" target="_blank" rel="noopener noreferrer">
    <img width="100" src="../public/assets/logo.png" alt="Rotor logo">
  </a>
</p>

<p align="center">
  <strong>适用于 Windows 和 macOS 的快速、轻量桌面工具箱。</strong>
</p>

<p align="center">
  <a href="../README.md">English</a>
  <span> | </span>
  <span>中文</span>
</p>

<div align="center">

[![GitHub License](https://img.shields.io/github/license/Horbin-Magician/rotor?style=flat)](../LICENSE)
[![GitHub Downloads](https://img.shields.io/github/downloads/Horbin-Magician/rotor/total?style=flat)](https://github.com/Horbin-Magician/rotor/releases)
![Windows Support](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)
![macOS Support](https://img.shields.io/badge/macOS-000000?style=flat&logo=apple&logoColor=white)

</div>

## 关于 Rotor

Rotor 将常用桌面工具集中到全局快捷键中，同时保持快速和低资源占用。目前提供文件
搜索、截图与图片置顶、本地截图 OCR，以及可配置的快捷操作。

## 功能

### 文件搜索

- macOS 按 `Cmd+Shift+F`，Windows 按 `Ctrl+Shift+F` 打开搜索窗口。
- 输入文件名即时搜索，并通过键盘切换结果。
- 按 `Enter` 打开结果，也可以通过条目操作在文件夹中显示。
- Windows 支持以管理员身份打开适用的结果。
- 可在设置中配置需要排除的目录名称或路径。

<p align="center">
  <img src="./search_demo.png" width="500" alt="Rotor 文件搜索">
</p>

### 截图与图片置顶

- macOS 按 `Cmd+Shift+S`，Windows 按 `Ctrl+Shift+S` 开始截图。
- 在连接的显示器上框选区域，并将结果置顶显示。
- 使用画笔、矩形、箭头和文字标注置顶图片。
- 对置顶图片执行本地 OCR，并选择识别出的文字。
- 支持缩放、保存、复制、隐藏、恢复和关闭置顶图片。

默认置顶窗口快捷键：

| 操作 | 快捷键 |
| --- | --- |
| 保存 | `S` |
| 复制 | `Enter` |
| 隐藏 | `H` |
| 关闭 | `Escape` |

<p align="center">
  <img src="./screenshot_demo.png" width="558" alt="Rotor 截图工具">
</p>

### 快捷操作

通过全局快捷键运行终端命令。可以在设置中新增、编辑、禁用和测试操作，并为每个
操作分配快捷键。Rotor 默认提供打开终端和系统文件管理器的快捷操作。

### 设置与诊断

- 自定义全局快捷键和置顶窗口快捷键。
- 支持跟随系统、浅色和深色主题，以及中文和英文界面。
- 配置截图保存方式和搜索排除目录。
- 查看内存占用、搜索索引状态和相关系统权限。
- 自动检查更新。

## 安装

从 [GitHub Releases](https://github.com/Horbin-Magician/rotor/releases/latest) 下载最新安装包：

- Windows：使用 NSIS 安装程序。
- macOS：使用 DMG 镜像。

macOS 截图功能需要授予“屏幕录制”权限。Windows 采用每台计算机安装模式，可能会
请求管理员权限。

## 开发

Rotor 使用 [Tauri 2](https://tauri.app/)、Rust 2021、Vue 3 和 TypeScript。

### 环境要求

- [Tauri 开发环境要求](https://v2.tauri.app/start/prerequisites/)
- Rust 和 Cargo
- Node.js 与 Yarn `1.22.22`

### 本地运行

```bash
yarn install
yarn tauri dev
```

仅启动前端开发服务器：

```bash
yarn dev
```

### 检查与构建

```bash
# 前端类型检查、代码检查、格式检查和构建
yarn typecheck
yarn lint
yarn format:check
yarn build

# Rust 工作区
cd src-tauri
cargo check --workspace
cargo test --workspace

# 平台应用安装包
cd ..
yarn tauri build
```

主要代码位于 `src/`（Vue 前端）和 `src-tauri/`（Tauri 应用及 Rust 工作区 crate）。

## 参与贡献

欢迎提交 Issue 和 Pull Request。请保持改动范围清晰，运行相关的前端和 Rust 检查，
并在受影响的操作系统上测试平台相关功能。

## 开源协议

Rotor 基于 [MIT License](../LICENSE) 开源。

Copyright (c) 2024-present Horbin
