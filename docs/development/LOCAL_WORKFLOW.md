# 本地开发工作流

本项目使用同一组 `package.json` scripts 作为本地和自动化任务入口；托管 CI
不能替代本地验证。所有命令都从仓库根目录运行。新增功能或改变现有行为前，先使用
[功能开发检查清单](FEATURE_PROCESS.md)确认边界、生命周期和验证范围。

## 开发环境

- Node.js `22.22.0` 或更高版本
- pnpm `11.20.0`
- Rust `1.94.0`（由根目录 `rust-toolchain.toml` 固定，Rust 2021 edition）

## 命令分组

| 目的                | 聚合命令            | 单栈命令                                       |
| ------------------- | ------------------- | ---------------------------------------------- |
| 安装或同步依赖      | `pnpm install`      | —                                              |
| 启动 Tauri 桌面应用 | `pnpm dev`          | —                                              |
| 构建桌面安装包      | `pnpm build`        | —                                              |
| 类型与编译检查      | `pnpm check`        | `pnpm check:ts`、`pnpm check:rs`               |
| 静态检查            | `pnpm lint`         | `pnpm lint:ts`、`pnpm lint:rs`                 |
| 测试                | `pnpm test`         | `pnpm test:ts`、`pnpm test:rs`                 |
| 写入格式化          | `pnpm format`       | `pnpm format:ts`、`pnpm format:rs`             |
| 只读格式检查        | `pnpm format:check` | `pnpm format:check:ts`、`pnpm format:check:rs` |
| 完整交付门禁        | `pnpm run ci`       | —                                              |

`dev` 和 `build` 只表示完整 Tauri 应用入口。`src-tauri/tauri.conf.json` 的
`beforeDevCommand` 和 `beforeBuildCommand` 直接调用 Vite，不能回调这两个
scripts，否则会形成递归。

TypeScript 类型检查由 `tsc` 负责；JavaScript/TypeScript lint 使用 Oxlint，
格式化使用 Oxfmt。Vitest 在继承默认排除规则的基础上忽略 `.worktrees/**`，
主工作区测试和 CI 不扫描隔离 worktree。Rust scripts 使用 `--workspace`/`--all`
覆盖 `yssbi` 和 `yss-sci`，并保持 Cargo 默认构建和链接并行度，不固定 build jobs。

Rust 测试使用 Cargo 内置 test runner。仓库不固定 build jobs 或 test threads，
`pnpm test:rs` 继承 Cargo 与 libtest 的默认并发。

`pnpm run ci` 按顺序执行格式检查、TypeScript/Rust 检查、Oxlint/Clippy 和完整
TypeScript/Rust 测试。必须保留 `run`：裸 `pnpm ci` 是 pnpm 的冻结安装命令，
不会执行同名 package script。该门禁不会启动应用或构建安装包；交付前仍需单独
运行 `git diff --check`。Oxfmt 和严格 Clippy 首次接入时会暴露既有基线问题，
不要为了让门禁表面通过而降低规则；应在独立任务中建立格式和 lint 基线。

`pnpm format` 会写入整个仓库，不要把它作为无关改动的顺手操作。验证时优先使用
`pnpm format:check`。

## 聚焦测试

聚焦测试通过单栈 script 向 `cargo test` 透传参数，Cargo 从根目录使用统一
workspace 和 `src-tauri/target/`：

```sh
pnpm test:ts src/path/to/example.test.ts
pnpm test:ts src/path/to/example.test.ts -t "test name"
pnpm test:rs --lib completed_task_has_terminal_status
pnpm test:rs --test database_test test_duckdb_query_page_and_schema_without_full_load
pnpm test:rs -p yss-sci test_name
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

## 按改动范围验证

- **React、TypeScript、样式或前端状态改动：**
  运行 `pnpm format:check:ts`、`pnpm check:ts`、`pnpm lint:ts` 和受影响的
  `pnpm test:ts` 测试。
- **Rust、Tauri command、项目状态或执行引擎改动：**
  先添加或更新聚焦回归测试，运行 `pnpm format:check:rs`、`pnpm check:rs`、
  `pnpm lint:rs` 和受影响的 `pnpm test:rs` 测试。
- **跨前后端、发布或执行引擎跨切面改动：**
  运行 `pnpm run ci`；Tauri 打包、权限、插件或构建配置改动还需运行 `pnpm build`
  并手动验证关键路径。

## 一次性 Cargo 维护命令

只在构建缓存损坏、依赖切换或需要释放磁盘空间时，从仓库根目录运行：

```sh
cargo clean --manifest-path src-tauri/Cargo.toml
```

所有一次性 Cargo 命令都必须显式使用 `src-tauri/Cargo.toml`。清理后下一次
Rust 构建会重新编译依赖，不要把 clean 纳入日常验证。
