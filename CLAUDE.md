# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 常用命令

```bash
cargo build                   # 调试构建
cargo build --release         # 发布构建
cargo run -- [参数]           # 运行（例如：cargo run -- --json /path/to/repo）
cargo test                    # 运行测试
cargo clippy                  # 代码检查
cargo fmt                     # 代码格式化
```

**运行所需的环境变量：**
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
```

## 代码架构

这是一个单二进制 Rust CLI 项目（`src/main.rs`），没有 library crate，所有逻辑集中在一个文件中。

**执行流程：**
1. 解析 CLI 参数（`Args` 结构体，通过 `clap`）— 支持 `--json`、`--brand`、`--verbose`，以及可选的 `repo_path`
2. 收集 git 上下文（`DiffContext`）— 通过 `tokio::process::Command` 执行 `git` 子进程，获取仓库名、暂存区 diff（统计摘要 + 完整内容）、当前分支、最近 10 条提交记录
3. 渲染提示词 — `DiffContext` 是一个 Askama 模板结构体，`templates/prompt.md` 会注入 git 上下文后渲染
4. 调用 Claude API — 通过 `rig-core` 的 Anthropic provider，使用模型 `claude-sonnet-4-6`，系统提示词为中文硬编码
5. 输出 — 默认输出纯文本，传入 `--json` 时输出 JSON（同时提示词模板也会指示 Claude 输出压缩 JSON）

**关键结构体：**
- `Args`：CLI 接口；`api_key` 和 `base_url` 优先从环境变量 `ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN` 和 `ANTHROPIC_BASE_URL` 读取，可通过 CLI 参数覆盖
- `DiffContext`：Askama 模板数据结构；`json_output` 字段同时控制渲染后的提示词格式（JSON 还是纯文本）

**模板文件：** `templates/prompt.md` — 完整的 Claude 提示词，以中文编写。要求 Claude 遵循 Conventional Commits 规范（`[type](scope): description`），标题不超过 72 字符，使用中文输出，并在 `json_output` 为 true 时输出压缩 JSON。

## 代码规范

- 提交信息遵循 `[type](scope): description` 格式（参考近期提交记录）
- 所有面向用户的字符串和提示词模板均使用中文
- 错误处理统一使用 `anyhow::Result`；git 命令错误会直接透传 stderr 内容

## 项目配置

- 项目所有使用的依赖项由开发者维护，AI 不能擅自修改依赖项