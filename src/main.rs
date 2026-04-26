use std::{ffi::OsStr, io::Write, path::Path, process::ExitStatus};

use anyhow::{Ok, Result};
use askama::Template;
use clap::Parser;
use inquire::Select;
use rig::{client::CompletionClient, completion::Prompt, providers::anthropic};
use tempfile::NamedTempFile;
use tokio::process::Command;

async fn commit_with_review(mut message: String, repo_path: &str) -> Result<()> {
    loop {
        println!("\n{}\n{}\n{}", "─".repeat(60), message, "─".repeat(60));

        let choice =
            Select::new("请选择操作:", vec!["✅ 接受并提交", "❌ 拒绝", "✏️  修改"]).prompt()?;

        match choice {
            "✅ 接受并提交" => {
                let tmp_path = {
                    let mut f = NamedTempFile::new()?;
                    write!(f, "{}", message)?;
                    f.flush()?;
                    f.into_temp_path()
                };
                let status = Command::new("git")
                    .current_dir(repo_path)
                    .arg("commit")
                    .arg("-F")
                    .arg(&tmp_path)
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("git commit 失败");
                }
                return Ok(());
            }
            "❌ 拒绝" => {
                println!("已取消提交");
                return Ok(());
            }
            "✏️  修改" => {
                let (editor, _) = run_git_command(repo_path, ["var", "GIT_EDITOR"]).await?;
                let mut editor_parts = editor.split_whitespace();
                let editor_bin = editor_parts.next().unwrap_or("vi");
                let editor_args: Vec<&str> = editor_parts.collect();
                let tmp_path = {
                    let mut f = NamedTempFile::new()?;
                    write!(f, "{}", message)?;
                    f.flush()?;
                    f.into_temp_path()
                };
                let exit = Command::new(editor_bin)
                    .args(&editor_args)
                    .arg(&tmp_path)
                    .status()
                    .await?;
                if exit.success() {
                    let candidate = std::fs::read_to_string(&tmp_path)?.trim().to_string();
                    if candidate.is_empty() {
                        println!("提交信息不能为空，已保留原始内容");
                    } else {
                        message = candidate;
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

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

    /// 禁用品牌信息输出（默认开启）
    #[clap(long = "no-brand", action = clap::ArgAction::SetFalse)]
    brand: bool,

    /// 输出 verbose 信息
    #[clap(short, long, action = clap::ArgAction::SetTrue)]
    verbose: bool,

    /// 生成后进入交互式 review 并直接提交
    #[clap(short = 'c', long, action = clap::ArgAction::SetTrue)]
    commit: bool,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.commit && args.json {
        eprintln!("❌ --commit 与 --json 不能同时使用");
        std::process::exit(1);
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

    let spinner_handle = if args.brand {
        Some(tokio::spawn(async {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            loop {
                print!("\r🤖 正在调用 Claude 生成 commit 信息... {}", frames[i % frames.len()]);
                let _ = std::io::stdout().flush();
                i += 1;
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            }
        }))
    } else {
        None
    };

    let response = agent.prompt(prompt).await?;

    if let Some(handle) = spinner_handle {
        handle.abort();
        let _ = handle.await;
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
    }

    if args.commit {
        commit_with_review(response, &repo_path).await?;
    } else {
        println!("{}", &response);
    }

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
