mod dag;
mod deps;
mod git;
mod job;
#[cfg(feature = "telemetry")]
mod telemetry;
mod update;
mod util;

use anyhow::{bail, Context, Result};
use clap_complete::generate;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
};
#[cfg(feature = "telemetry")]
use telemetry::{segment::TrackEvent, segment_enabled, sentry::sentry_init};
use update::check_for_update;
use url::Url;

use ahash::HashMap;
use clap::Parser;
use owo_colors::OwoColorize;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::{
    dag::{invert_graph, topological_sort, Node},
    deps::{download_cicada_musl, download_deno},
    git::github_repo,
    job::{OnFail, Pipeline},
};

const LOCAL_CLI_SCRIPT: &str = include_str!("../scripts/local-cli.ts");
const RUNNER_CLI_SCRIPT: &str = include_str!("../scripts/runner-cli.ts");

const DENO_VERSION: &str = "1.32.3";

const COLORS: [owo_colors::colored::Color; 6] = [
    owo_colors::colored::Color::Blue,
    owo_colors::colored::Color::Green,
    owo_colors::colored::Color::Red,
    owo_colors::colored::Color::Magenta,
    owo_colors::colored::Color::Cyan,
    owo_colors::colored::Color::Yellow,
];

fn print_error(s: impl std::fmt::Display) {
    eprintln!(
        "{}: {s}",
        "Error".if_supports_color(atty::Stream::Stderr, |c| c.red()),
    );
}

async fn run_deno<I, S>(script: &str, args: I) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new("deno");
    // TODO: maybe less perms here?
    command
        .arg("run")
        .arg("-A")
        .arg("-")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = command.spawn().context("Failed to spawn deno")?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())
        .await
        .context("Failed to write to deno stdin")?;

    child.wait().await.context("Failed to wait for deno")?;
    Ok(())
}

async fn run_deno_builder<A, S>(
    script: &str,
    args: A,
    proj_path: &Path,
    out_path: &Path,
) -> Result<()>
where
    A: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new("deno")
        .arg("run")
        .arg(format!("--allow-read={}", proj_path.display()))
        .arg(format!("--allow-write={}", out_path.display()))
        .arg("--allow-net")
        .arg("-")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Failed to spawn deno")?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(script.as_bytes())
        .await
        .context("Failed to write to deno stdin")?;
    // Close stdin so deno can exit
    drop(child.stdin.take());

    let output = child.wait().await.context("Failed to wait for deno")?;

    if !output.success() {
        anyhow::bail!("Failed to run deno script");
    }

    Ok(())
}

/// Check that docker is installed and buildx is installed, other checks before running cicada can be added here
async fn runtime_checks() {
    // Validate docker client version is at least 23
    match Command::new("docker")
        .args(["buildx", "version"])
        .output()
        .await
    {
        Ok(output) => {
            if !output.status.success() {
                print_error("Docker buildx is required to run cicada");
                println!("Update docker if you are using Docker Desktop or install buildx if you are on Linux");
                std::process::exit(1);
            }

            let version_str = String::from_utf8_lossy(&output.stdout);

            let version_str_parts = version_str.split_whitespace().collect::<Vec<&str>>();

            if version_str_parts[0] != "github.com/docker/buildx" {
                print_error("Docker buildx is required to run cicada");
                std::process::exit(1);
            }

            let version = version_str_parts[1]
                .strip_prefix('v')
                .unwrap_or(version_str_parts[1]);
            let version_parts = version.split('.').collect::<Vec<&str>>();

            let major = version_parts[0].parse::<u32>().unwrap_or_default();
            let minor = version_parts[1].parse::<u32>().unwrap_or_default();

            if major == 0 && minor < 9 {
                print_error("Buildx version 0.9 or higher is required to run cicada");
                std::process::exit(1);
            }
        }
        Err(_) => {
            print_error("Docker is required to run cicada");
            std::process::exit(1);
        }
    }
}

pub fn resolve_cicada_dir() -> Result<PathBuf> {
    let mut path = std::env::current_dir()?;

    loop {
        let cicada_path = path.join(".cicada");
        if cicada_path.exists() {
            return Ok(cicada_path);
        }

        match path.parent() {
            Some(parent) => path = parent.to_path_buf(),
            None => return Err(anyhow::anyhow!("Could not find cicada.yml")),
        }
    }
}

pub fn resolve_pipeline(pipeline: impl AsRef<Path>) -> Result<PathBuf> {
    let pipeline = pipeline.as_ref();
    if pipeline.is_file() {
        // Check that the parent is a cicada dir
        let cicada_dir = pipeline.parent().expect("Could not get parent");
        if cicada_dir.ends_with(".cicada") {
            return Ok(pipeline.canonicalize()?);
        } else {
            anyhow::bail!("Pipeline must be in the .cicada directory");
        }
    }

    let cicada_dir = resolve_cicada_dir()?;
    let pipeline_path = cicada_dir.join(pipeline).with_extension("ts");
    if pipeline_path.exists() {
        return Ok(pipeline_path);
    }

    Err(anyhow::anyhow!("Could not find pipeline"))
}

#[derive(Parser)]
#[command(author, version, about)]
enum Commands {
    /// Run a cicada pipeline
    Run {
        /// Path to the pipeline file
        pipeline: PathBuf,
        /// Name of the secret to use, these come from environment variables
        secrets: Vec<String>,
    },
    /// Run a step in a cicada workflow
    #[command(hide = true)]
    Step { workflow: usize, step: usize },
    /// Initialize a cicada project, you can optionally specify a pipeline to create
    Init { pipeline: Option<String> },
    /// Create a cicada pipeline
    New { pipeline: String },
    /// Update cicada
    Update,
    /// Download all dependencies needed for runtime
    #[command(hide = true)]
    DownloadDeps,
    /// List all available completions
    Completions { shell: clap_complete::Shell },
}

impl Commands {
    async fn execute(self) -> anyhow::Result<()> {
        match self {
            Commands::Run { pipeline, secrets } => {
                tokio::join!(check_for_update(), runtime_checks());

                eprintln!();
                eprintln!(
                    "{}{}",
                    " ◥◣ ▲ ◢◤ "
                        .if_supports_color(atty::Stream::Stderr, |s| s.fg_rgb::<145, 209, 249>()),
                    " Cicada is in alpha, it may not work as expected"
                        .if_supports_color(atty::Stream::Stderr, |s| s.bold())
                );
                eprintln!(
                    "{}{}",
                    "  ◸ ▽ ◹  "
                        .if_supports_color(atty::Stream::Stderr, |s| s.fg_rgb::<145, 209, 249>()),
                    " Please report any issues here: http://github.com/cicadahq/cicada"
                        .if_supports_color(atty::Stream::Stderr, |s| s.bold())
                );
                eprintln!();

                let pipeline = resolve_pipeline(pipeline)?;
                let pipeline_file_name = pipeline.file_name().unwrap();
                let project_dir = pipeline.parent().unwrap().parent().unwrap();
                let pipeline_url = Url::from_file_path(&pipeline)
                    .map_err(|_| anyhow::anyhow!("Unable to convert pipeline path to URL"))?;

                // set the cwd to the parent of the .cicada directory
                std::env::set_current_dir(pipeline.parent().unwrap().parent().unwrap())?;

                let (deno_exe, cicada_musl_exe) =
                    tokio::try_join!(download_deno(), download_cicada_musl())?;

                let deno_dir = deno_exe.parent().unwrap();
                let cicada_musl_dir = cicada_musl_exe.parent().unwrap();

                let gh_repo = github_repo().await.ok().flatten();

                println!("Building pipeline: {}", pipeline.display());

                let out = {
                    let tmp_file = tempfile::NamedTempFile::new()?;

                    run_deno_builder(
                        LOCAL_CLI_SCRIPT,
                        vec![
                            pipeline_url.to_string().as_ref(),
                            tmp_file.path().to_str().unwrap(),
                        ],
                        project_dir,
                        tmp_file.path(),
                    )
                    .await?;

                    // Read the output file
                    std::fs::read_to_string(tmp_file.path())?
                };

                let deser = serde_json::from_str::<Pipeline>(&out)?;
                let mut jobs = HashMap::from_iter(
                    deser
                        .jobs
                        .into_iter()
                        .enumerate()
                        .map(|(index, job)| (job.uuid, (index, job))),
                );

                let nodes: Vec<Node> = jobs
                    .values()
                    .map(|(_, job)| Node::new(job.uuid, job.depends_on.to_vec()))
                    .collect();
                let graph = topological_sort(&invert_graph(&nodes))?;

                for run_group in graph {
                    match futures::future::try_join_all(run_group.into_iter().map(|job| {
                        let (job_index, job) = jobs.remove(&job).unwrap();
                        let secrets = secrets.clone();
                        let gh_repo = gh_repo.clone();
                        let cicada_musl_dir = cicada_musl_dir.to_path_buf();
                        let deno_dir = deno_dir.to_path_buf();
                        let pipeline_file_name = pipeline_file_name.to_os_string();
                        let project_dir = project_dir.to_path_buf();

                        tokio::spawn(async move {
                            let tag = format!("cicada-{}", job.image);

                            let mut args: Vec<String> = vec![
                                "buildx".into(),
                                "build".into(),
                                "-t".into(),
                                tag,
                                "--build-context".into(),
                                format!("local={}", project_dir.to_str().unwrap()),
                                "--build-context".into(),
                                format!("cicada-bin={}", cicada_musl_dir.to_str().unwrap()),
                                "--build-context".into(),
                                format!("deno-bin={}", deno_dir.to_str().unwrap()),
                                "--progress".into(),
                                "plain".into(),
                            ];

                            for secret in &secrets {
                                args.extend(["--secret".into(), format!("id={secret}")]);
                            }

                            args.push("-".into());

                            let mut buildx = Command::new("docker")
                                .args(args)
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped())
                                .spawn()?;

                            let dockerfile = job.to_dockerfile(
                                pipeline_file_name.to_str().unwrap(),
                                &gh_repo,
                                job_index,
                            );

                            buildx
                                .stdin
                                .as_mut()
                                .unwrap()
                                .write_all(dockerfile.as_bytes())
                                .await?;
                            buildx.stdin.take().unwrap().shutdown().await?;

                            let display_name =
                                job.name.clone().unwrap_or_else(|| job.image.clone());

                            // Print the output as it comes in
                            let stdout = buildx.stdout.take().unwrap();
                            let stderr = buildx.stderr.take().unwrap();

                            // TODO: Make this into a function that takes a stream, a color, and a display name
                            let stdout_handle = tokio::spawn({
                                let display_name = display_name.clone();

                                async move {
                                    let mut buf_reader = BufReader::new(stdout);
                                    let mut line = String::new();
                                    loop {
                                        if let Err(err) = buf_reader.read_line(&mut line).await {
                                            print_error(err);
                                            return;
                                        }
                                        if line.is_empty() {
                                            return;
                                        }
                                        print!(
                                            "{}: {line}",
                                            display_name
                                                .if_supports_color(atty::Stream::Stdout, |s| s
                                                    .color(COLORS[job_index % COLORS.len()])),
                                        );
                                        line.clear();
                                    }
                                }
                            });

                            let stderr_handle = tokio::spawn({
                                let display_name = display_name.clone();

                                async move {
                                    let mut buf_reader = BufReader::new(stderr);
                                    let mut line = String::new();
                                    loop {
                                        if let Err(err) = buf_reader.read_line(&mut line).await {
                                            print_error(err);
                                            return;
                                        }
                                        if line.is_empty() {
                                            return;
                                        }
                                        print!(
                                            "{}: {line}",
                                            display_name
                                                .if_supports_color(atty::Stream::Stderr, |s| s
                                                    .color(COLORS[job_index % COLORS.len()])),
                                        );
                                        line.clear();
                                    }
                                }
                            });

                            stdout_handle.await.with_context(|| {
                                format!("Failed to read stdout for {display_name}")
                            })?;
                            stderr_handle.await.with_context(|| {
                                format!("Failed to read stderr for {display_name}")
                            })?;

                            let status = buildx.wait().await.with_context(|| {
                                format!("Failed to wait for {display_name} to finish")
                            })?;

                            anyhow::Ok((display_name, status, job))
                        })
                    }))
                    .await
                    {
                        Ok(results) => {
                            for result in results {
                                match result {
                                    Ok((display_name, exit_status, job)) => match job.on_fail {
                                        Some(OnFail::Ignore) if !exit_status.success() => {
                                            println!("{display_name} failed with status {exit_status} but was ignored");
                                        }
                                        Some(OnFail::Stop) | None if !exit_status.success() => {
                                            print_error(format!(
                                                    "Docker build failed for \"{display_name}\" with status {exit_status}",
                                                ));
                                            std::process::exit(1);
                                        }
                                        _ => {
                                            println!(
                                                "{display_name} finished with status {exit_status}"
                                            );
                                        }
                                    },
                                    Err(err) => {
                                        print_error(err);
                                        std::process::exit(1);
                                    }
                                }
                            }
                        }
                        Err(e) => bail!(e),
                    }
                }
            }
            Commands::Step { workflow, step } => {
                run_deno(
                    RUNNER_CLI_SCRIPT,
                    vec![workflow.to_string(), step.to_string()],
                )
                .await?;
            }
            Commands::Init { pipeline } => {
                check_for_update().await;

                // if std::env::var("TERM_PROGRAM").as_deref() == Ok("vscode") {
                //     let bin_name = match std::env::var("TERM_PROGRAM_VERSION") {
                //         Ok(version) if version.contains("insider") => "code-insiders",
                //         _ => "code",
                //     };

                //     // Check if deno extension is installed
                //     let deno_extension_installed = String::from_utf8_lossy(
                //         &Command::new(bin_name)
                //             .args(&["--list-extensions"])
                //             .output()
                //             .await?
                //             .stdout,
                //     )
                //     .contains("denoland.vscode-deno");

                //     if !deno_extension_installed {
                //         println!("Installing Deno extension for VSCode");
                //         Command::new(bin_name)
                //             .args(&["--install-extension", "denoland.vscode-deno"])
                //             .spawn()
                //             .unwrap()
                //             .wait()
                //             .await?;
                //     }
                // }

                let cicada_dir = PathBuf::from(".cicada");

                if !cicada_dir.exists() {
                    std::fs::create_dir(&cicada_dir)?;
                }

                let pipeline_name = pipeline.as_deref().unwrap_or("my-pipeline");

                let pipeline_path = cicada_dir.join(format!("{pipeline_name}.ts"));

                if pipeline_path.exists() {
                    print_error(format!(
                        "Pipeline already exists: {}",
                        pipeline_path.display()
                    ));
                    std::process::exit(1);
                }

                tokio::fs::write(&pipeline_path, include_str!("default-pipeline.ts")).await?;

                // Cache deno dependencies
                if let Ok(mut out) = Command::new("deno")
                    .args(["cache", "-q", pipeline_path.to_str().unwrap()])
                    .spawn()
                {
                    out.wait().await.ok();
                }

                println!();
                println!(
                    "{} Initialized Cicada pipeline: {}",
                    " ◥◣ ▲ ◢◤ ".fg_rgb::<145, 209, 249>(),
                    pipeline_name.bold(),
                );
                println!(
                    "{} Run it with: {}",
                    "  ◸ ▽ ◹  ".fg_rgb::<145, 209, 249>(),
                    format!("cicada run {pipeline_name}").bold(),
                );
                println!();
            }
            Commands::New { pipeline } => {
                // Check if cicada is initialized
                if !PathBuf::from(".cicada").exists() {
                    print_error(format!(
                        "Cicada is not initialized in this directory. Run {} to initialize it.",
                        "cicada init".bold()
                    ));
                    std::process::exit(1);
                }

                let pipeline_path = PathBuf::from(".cicada").join(format!("{pipeline}.ts"));

                if pipeline_path.exists() {
                    print_error(format!(
                        "Pipeline already exists: {}",
                        pipeline_path.display()
                    ));
                    std::process::exit(1);
                }

                tokio::fs::write(&pipeline_path, include_str!("default-pipeline.ts")).await?;

                println!();
                println!(
                    "{} Initialized Cicada pipeline: {}",
                    " ◥◣ ▲ ◢◤ ".fg_rgb::<145, 209, 249>(),
                    pipeline.bold(),
                );
                println!(
                    "{} Run it with: {}",
                    "  ◸ ▽ ◹  ".fg_rgb::<145, 209, 249>(),
                    format!("cicada run {pipeline}").bold(),
                );
                println!();
            }
            #[cfg(not(target_env = "musl"))]
            Commands::Update => {
                use update::self_update_release;

                let status =
                    tokio::task::spawn_blocking(move || -> anyhow::Result<self_update::Status> {
                        let status = self_update_release()?.update()?;
                        Ok(status)
                    })
                    .await??;

                match status {
                    self_update::Status::UpToDate(ver) => {
                        println!();
                        println!("Already up to date: {ver}");
                    }
                    self_update::Status::Updated(ver) => {
                        println!();
                        println!("Updated to version {ver}");
                    }
                }
            }
            #[cfg(target_env = "musl")]
            Commands::Update => {
                print_error("Updates are not supported on musl");
                std::process::exit(1);
            }
            Commands::DownloadDeps => {
                tokio::try_join!(download_deno(), download_cicada_musl())?;
            }
            Commands::Completions { shell } => {
                use clap::CommandFactory;
                generate(
                    shell,
                    &mut Commands::command(),
                    "cicada",
                    &mut std::io::stdout(),
                );
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn subcommand(&self) -> &'static str {
        match self {
            Commands::Run { .. } => "run",
            Commands::Step { .. } => "step",
            Commands::Init { .. } => "init",
            Commands::New { .. } => "new",
            Commands::Update => "update",
            Commands::DownloadDeps => "download_deps",
            Commands::Completions { .. } => "completions",
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(feature = "telemetry")]
    let _sentry_guard = sentry_init();

    if std::env::var_os("CICADA_FORCE_COLOR").is_some() {
        owo_colors::set_override(true);
    }

    let command = Commands::parse();

    #[cfg(feature = "telemetry")]
    let telem_join = segment_enabled().then(|| {
        let subcommand = command.subcommand().to_owned();
        tokio::spawn(
            TrackEvent::SubcommandExecuted {
                subcommand_name: subcommand,
            }
            .post(),
        )
    });

    let res = command.execute().await;

    #[cfg(feature = "telemetry")]
    if let Some(join) = telem_join {
        join.await.ok();
    }

    res?;

    Ok(())
}
