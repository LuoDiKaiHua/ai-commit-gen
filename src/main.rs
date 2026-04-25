use std::{ffi::OsStr, path::Path, process::ExitStatus};

use anyhow::{Ok, Result};
use askama::Template;
use clap::Parser;
use rig::{client::CompletionClient, completion::Prompt, providers::anthropic};
use tokio::process::Command;

fn resolve_env(names: &Vec<&str>) -> Option<String> {
    names.iter().find_map(|key| std::env::var(key).ok())
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// 目标 git 仓库路径
    repo_path: Option<String>,

    /// AI 接口的 base url
    #[clap(short, long, default_value_t = resolve_env(&vec!("ANTHROPIC_BASE_URL")).unwrap_or_default())]
    base_url: String,

    /// AI 接口的 api key
    #[clap(short, long, default_value_t = resolve_env(&vec!("ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN")).unwrap_or_default())]
    api_key: String,

    /// 是否输出 json 结果
    #[clap(short, long, action = clap::ArgAction::SetTrue)]
    json: bool,

    /// 是否输出品牌信息
    #[clap(long, action = clap::ArgAction::SetTrue)]
    brand: bool,

    /// 输出 verbose 信息
    #[clap(short, long, action = clap::ArgAction::SetTrue)]
    verbose: bool,
}

#[derive(Debug, Template)]
#[template(path = "prompt.md")]
struct DiffContext {
    project_name: String,
    staged_diff: String,
    staged_diff_detail: String,
    branch: String,
    recent_commits: String,
    json_output: bool,
}

impl DiffContext {
    async fn new(repo_path: &str, json_output: bool) -> Result<DiffContext> {
        Ok(Self {
            project_name: get_repo_name(repo_path).await?,
            staged_diff: get_staged_diff(repo_path, Some(false)).await?,
            staged_diff_detail: get_staged_diff(repo_path, Some(true)).await?,
            branch: get_current_branch(repo_path).await?,
            recent_commits: get_recent_commits(repo_path, None).await?,
            json_output,
        })
    }
}

#[allow(dead_code, unused_variables)]
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.brand {
        println!("🤖 正在调用 Claude 生成 commit 信息...")
    }

    let is_verbose = args.verbose;

    if is_verbose && !args.base_url.is_empty() && !args.api_key.is_empty() {
        println!("🌐 使用的 base url:\t\"{}\"", args.base_url);
        println!("🔑 使用的 api token:\t\"{}\"", args.api_key);
    }

    let repo_path = (args.repo_path).clone().unwrap_or_else(|| {
        std::env::current_dir()
            .expect("无法获取当前目录")
            .to_str()
            .expect("路径包含无效的 UTF-8 字符")
            .to_string()
    });

    if is_verbose {
        println!("📁 目标仓库路径:\t\"{}\"", repo_path);
    }

    if args.base_url.is_empty() {
        panic!("❌ base url 不能为空，请通过命令行参数或环境变量提供有效的 base url");
    }

    if args.api_key.is_empty() {
        panic!("❌ api key 不能为空，请通过命令行参数或环境变量提供有效的 api key");
    }

    let context = DiffContext::new(&repo_path, args.json).await?;

    if is_verbose {
        println!("📊 获取到的上下文信息:\n{:#?}", context);
    }

    if context.staged_diff.is_empty() {
        println!("⚠️ 没有检测到 staged changes, 请先使用 git add 添加更改到暂存区");
        return Ok(());
    }

    let prompt = context.render().unwrap();
    if is_verbose {
        println!("生成的 prompt:\n{}", prompt);
    }

    let client = anthropic::Client::builder()
        .base_url(args.base_url)
        .api_key(args.api_key)
        .build()?;

    let agent = client
        .agent("claude-sonnet-4-6")
        .preamble(
            "你是一个 git commit message 生成器，帮助用户根据 git diff 生成规范的 commit message",
        )
        .build();

    let response = agent.prompt(prompt).await?;

    println!("{}", &response);

    Ok(())
}

async fn run_git_command<I, S>(repo_path: &str, args: I) -> Result<(String, ExitStatus)>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .output()
        .await?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok((stdout, output.status))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow::anyhow!("Git 命令执行失败: {}", stderr))
    }
}

async fn get_repo_name(repo_path: &str) -> Result<String> {
    let (output, status) =
        run_git_command(repo_path, "rev-parse --show-toplevel".split(" ")).await?;

    if status.success() {
        let repo_name = Path::new(&output)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未知仓库")
            .to_string();
        Ok(repo_name)
    } else {
        Err(anyhow::anyhow!("获取仓库名称失败: {}", output))
    }
}

async fn get_staged_diff(repo_path: &str, is_detail: Option<bool>) -> Result<String> {
    let take_detail = is_detail.unwrap_or(false);

    let args = if take_detail {
        "diff --cached".split(" ")
    } else {
        "diff --cached --stat".split(" ")
    };

    let (output, status) = run_git_command(repo_path, args).await?;

    if status.success() {
        Ok(output)
    } else {
        Err(anyhow::anyhow!("获取 staged diff 失败: {}", output))
    }
}

async fn get_current_branch(repo_path: &str) -> Result<String> {
    let (output, status) = run_git_command(repo_path, "branch --show-current".split(" ")).await?;

    if status.success() {
        Ok(output)
    } else {
        Err(anyhow::anyhow!("获取当前分支失败: {}", output))
    }
}

async fn get_recent_commits(repo_path: &str, count: Option<u32>) -> Result<String> {
    let count = count.unwrap_or(10);

    let result = run_git_command(repo_path, format!("log --oneline -{}", count).split(" ")).await;

    Ok(result
        .map(|(o, _)| o)
        .unwrap_or("暂无历史 commit".to_string()))
}
