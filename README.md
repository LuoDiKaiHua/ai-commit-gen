# ai-commit-gen

基于 Claude AI 的 Git commit 信息生成器。自动分析暂存区的改动，生成符合 [Conventional Commits](https://www.conventionalcommits.org/) 规范的提交信息。

## 功能

- 自动读取暂存区 diff，结合项目名称、当前分支和近期提交历史生成 commit 信息
- 遵循 `[type](scope): description` 格式，输出中文提交信息
- `--commit` 交互式 review 模式：接受、拒绝或在编辑器中修改后直接提交
- `--json` 输出 JSON 格式，方便脚本集成
- `--verbose` 输出调试信息

## 环境要求

- Rust 工具链（edition 2024）
- Git
- Anthropic API Key

## 安装

```bash
cargo build --release
# 可执行文件位于 target/release/ai-commit-gen
```

## 配置

通过环境变量提供 API 凭据：

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_BASE_URL="https://api.anthropic.com"
```

也可以通过命令行参数 `-a`/`--api-key` 和 `-b`/`--base-url` 覆盖。

## 用法

```bash
# 在当前仓库生成 commit 信息（需先 git add）
ai-commit-gen

# 指定仓库路径
ai-commit-gen /path/to/repo

# 生成后进入交互式 review，可直接提交
ai-commit-gen --commit

# 输出 JSON 格式（方便脚本集成）
ai-commit-gen --json

# 禁用品牌 spinner 动画
ai-commit-gen --no-brand

# 输出调试信息
ai-commit-gen --verbose
```

### 输出示例

```
[feature](用户登录): 添加用户登录功能

  - 实现 JWT token 验证
  - 添加登录/登出接口
  - 更新用户状态管理
```

## 开源协议

[MIT](LICENSE)
