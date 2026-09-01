<div align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="YssSkills Logo" />
  <h1>YssSkills</h1>
  <p><strong>轻量 Agent Skill 桌面管理器</strong></p>
  <p>集中浏览本地 Skills、组合式管理(添加/删除)本地 Skills，查看工作区状态，并发现远程 Skill。</p>

[![下载最新版本](https://img.shields.io/badge/下载最新版本-GitHub_Releases-2EA44F?style=for-the-badge&logo=github)](https://github.com/zhouyi207/YssSkills/releases)

[![Publish](https://github.com/zhouyi207/YssSkills/actions/workflows/publish.yml/badge.svg)](https://github.com/zhouyi207/YssSkills/actions/workflows/publish.yml)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![React](https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=black)
![Rust](https://img.shields.io/badge/Rust-1.94-000000?logo=rust&logoColor=white)
</div>

> [!NOTE]
> 项目仍处于早期开发阶段，功能与数据格式可能持续调整。

## ✨ 功能

- 🧩 扫描并查看本地 Agent Skills
- 🤖 检测不同 Agent Harness 及其配置
- 🗂️ 查看 Agents、Project 与 Linked Workspace 状态
- 🔎 搜索远程 Skill Registry 与排行榜
- 💻 支持 Windows、macOS 和 Linux

## 📦 下载安装

前往 **[GitHub Releases](https://github.com/zhouyi207/YssSkills/releases)** 下载适用于 Windows、macOS 或 Linux 的最新安装包，并根据设备架构选择对应版本。

## 🚀 本地开发

环境要求：Node.js `22.22.0+`、pnpm `11.20.0`、Rust `1.94.0`。

```bash
git clone https://github.com/zhouyi207/YssSkills.git
cd YssSkills
pnpm install
pnpm dev
```

## 🛠️ 常用命令

| 命令          | 用途             |
| ------------- | ---------------- |
| `pnpm dev`    | 启动桌面应用     |
| `pnpm build`  | 构建安装包       |
| `pnpm test`   | 运行全部测试     |
| `pnpm run ci` | 执行完整质量检查 |

## 🧱 技术栈

[Tauri 2](https://tauri.app/) · [Rust](https://www.rust-lang.org/) · [React 19](https://react.dev/) · [TypeScript](https://www.typescriptlang.org/) · [Vite](https://vite.dev/) · [SQLite](https://www.sqlite.org/)

## 📚 文档

- [项目架构](docs/architecture/ARCHITECTURE.md)
- [本地开发](docs/development/LOCAL_WORKFLOW.md)
- [功能开发流程](docs/development/FEATURE_PROCESS.md)

## 🙏 参考项目

[skillspec](https://github.com/modiqo/skillspec) · [skills-hub](https://github.com/qufei1993/skills-hub) · [skills-manager](https://github.com/xingkongliang/skills-manager)
