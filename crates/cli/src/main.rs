mod dag;
mod deno;
mod git;
mod job;
mod logging;
mod oci;
#[cfg(feature = "telemetry")]
mod telemetry;
#[cfg(feature = "self-update")]
mod update;
mod util;

use anyhow::{bail, Context, Result};
use clap_complete::generate;
use logging::logging_init;
use oci::OciBackend;
use once_cell::sync::Lazy;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
};
#[cfg(feature = "telemetry")]
use telemetry::{segment::TrackEvent, segment_enabled, sentry::sentry_init};
use tracing::{error, info, info_span, warn, Instrument};
#[cfg(feature = "self-update")]
use update::check_for_update;
use url::Url;

use ahash::HashMap;
use clap::Parser;
use owo_colors::{OwoColorize, Stream};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::{
    dag::{invert_graph, topological_sort, Node},
    deno::deno_exe,
    git::github_repo,
    job::{OnFail, Pipeline},
};

// Transform from https://deno.land/x/cicada/lib.ts to https://deno.land/x/cicada@vX.Y.X/lib.ts
static DENO_LAND_REGEX: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r#"deno.land/x/cicada/"#).unwrap());

fn replace_with_version(s: &str) -> String {
    DENO_LAND_REGEX
        .replace_all(s, |_caps: &regex::Captures| {
            format!("deno.land/x/cicada@v{}/", env!("CARGO_PKG_VERSION"))
        })
        .into_owned()
}

static LOCAL_CLI_SCRIPT: Lazy<String> =
    Lazy::new(|| replace_with_version(include_str!("../scripts/local-cli.ts")));
static RUNNER_CLI_SCRIPT: Lazy<String> =
    Lazy::new(|| replace_with_version(include_str!("../scripts/runner-cli.ts")));
static DEFAULT_PIPELINE: Lazy<String> =
    Lazy::new(|| replace_with_version(include_str!("../scripts/default-pipeline.ts")));

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
    deno_exe: &Path,
    script: &str,
    args: A,
    proj_path: &Path,
    out_path: &Path,
) -> Result<()>
where
    A: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut child = Command::new(deno_exe)
        .arg("run")
        .arg(format!("--allow-read={}", proj_path.display()))
        .arg(format!("--allow-write={}", out_path.display()))
        .arg("--allow-net")
        .arg("--allow-env=CICADA_JOB")
        .arg("-")
        .args(args)
        .current_dir(proj_path)
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
async fn runtime_checks(oci: &OciBackend) {
    if std::env::var_os("CICADA_SKIP_CHECKS").is_some() {
        return;
    }

    // We dont do any checks for podman yet
    if oci == &OciBackend::Podman {
        return;
    }

    let (docker_buildx_version, docker_info) = tokio::join!(
        Command::new("docker").args(["buildx", "version"]).output(),
        Command::new("docker")
            .args(["info", "--format", "{{json .}}"])
            .output(),
    );

    // Validate docker client version is at least 23
    match docker_buildx_version {
        Ok(output) => {
            let buildx_error = || {
                if std::env::consts::OS == "macos" || std::env::consts::OS == "windows" {
                    info!("Cicada requires Docker Desktop v4.12 or above to run. Please upgrade using the Docker Desktop UI");
                } else {
                    info!("Cicada requires Docker Buildx >=0.9 to run. Please install it by updating Docker to v4.12 or by manually downloading from from https://github.com/docker/buildx#linux-packages");
                }
                std::process::exit(1);
            };

            if !output.status.success() {
                buildx_error();
            }

            let version_str = String::from_utf8_lossy(&output.stdout);

            let version_str_parts = version_str.split_whitespace().collect::<Vec<&str>>();

            if version_str_parts[0] != "github.com/docker/buildx" {
                buildx_error();
            }

            let version = version_str_parts[1]
                .strip_prefix('v')
                .unwrap_or(version_str_parts[1]);
            let version_parts = version.split('.').collect::<Vec<&str>>();

            let major = version_parts[0].parse::<u32>().unwrap_or_default();
            let minor = version_parts[1].parse::<u32>().unwrap_or_default();

            if major == 0 && minor < 9 {
                buildx_error();
            }
        }
        Err(_) => {
            error!("Cicada requires Docker to run. Please install it using one of the methods on install it from https://docs.docker.com/engine/install");
            std::process::exit(1);
        }
    }

    // Run docker info to check that docker is running
    match docker_info {
        Ok(output) => {
            if !output.status.success() {
                error!("Docker is not running! Please start it to use Cicada");
                std::process::exit(1);
            }
        }
        Err(_) => {
            error!("Cicada requires Docker to run. Please install it using one of the methods on https://docs.docker.com/engine/install");
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
            None => return Err(anyhow::anyhow!("Could not find .cicada directory")),
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

#[derive(Parser, Debug)]
#[command(name = "cicada", bin_name = "cicada", author, version, about)]
enum Commands {
    /// Run a cicada pipeline
    Run {
        /// Path to the pipeline file
        pipeline: PathBuf,

        /// Name of the secret to use, these come from environment variables
        ///
        /// The CLI will also look for a .env file
        #[arg(short, long)]
        secret: Vec<String>,

        /// Do not load .env file
        #[arg(long)]
        no_dotenv: bool,

        /// Load a custom .env file
        ///
        /// This will override the default .env lookup
        #[arg(long)]
        dotenv: Option<PathBuf>,

        /// Load secrets from a json file
        ///
        /// They should look like this:
        /// `{
        ///     "KEY": "VALUE",
        ///     "KEY2": "VALUE2"
        /// }`
        #[arg(long)]
        secrets_json: Option<PathBuf>,

        /// A custom dockerfile to load the cicada bin from
        ///
        /// In the dev reop this is `./docker/bin.Dockerfile`
        #[arg(long, hide = true)]
        cicada_dockerfile: Option<PathBuf>,

        /// The backend to use for OCI
        #[arg(long, default_value = "docker", env = "CICADA_OCI_BACKEND")]
        oci_backend: OciBackend,

        /// Use the new experimental buildkit backend, this requires the
        /// buildkit daemon to be running as `buildkitd` and the `buildctl` CLI
        #[arg(long)]
        buildkit_expiremental: bool,
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
    /// List all available completions
    Completions { shell: clap_complete::Shell },
    #[command(hide = true)]
    Doctor {
        /// The backend to use for OCI
        #[arg(long, default_value = "docker", env = "CICADA_OCI_BACKEND")]
        oci_backend: OciBackend,
    },
}

impl Commands {
    async fn execute(self) -> anyhow::Result<()> {
        match self {
            Commands::Run {
                pipeline,
                secret,
                no_dotenv,
                dotenv,
                secrets_json,
                cicada_dockerfile,
                oci_backend,
                buildkit_expiremental,
            } => {
                #[cfg(feature = "self-update")]
                tokio::join!(check_for_update(), runtime_checks(&oci_backend));

                #[cfg(not(feature = "self-update"))]
                runtime_checks(&oci_backend).await;

                info!(
                    "\n{}{}\n{}{}\n",
                    " ◥◣ ▲ ◢◤ ".if_supports_color(Stream::Stderr, |s| s.fg_rgb::<145, 209, 249>()),
                    " Cicada is in alpha, it may not work as expected"
                        .if_supports_color(Stream::Stderr, |s| s.bold()),
                    "  ◸ ▽ ◹  ".if_supports_color(Stream::Stderr, |s| s.fg_rgb::<145, 209, 249>()),
                    " Please report any issues here: http://github.com/cicadahq/cicada"
                        .if_supports_color(Stream::Stderr, |s| s.bold())
                );

                let deno_exe = deno_exe().await?;

                let cicada_bin_tag = if let Some(cicada_dockerfile) = cicada_dockerfile {
                    let tag = format!("cicada-bin:{}", env!("CARGO_PKG_VERSION"));

                    info!("Building cicada bootstrap image...\n");

                    let status = Command::new(oci_backend.as_str())
                        .arg("build")
                        .arg("-t")
                        .arg(&tag)
                        .arg("-f")
                        .arg(cicada_dockerfile)
                        .arg(".")
                        .spawn()?
                        .wait()
                        .await
                        .map_err(|err| {
                            anyhow::anyhow!("Unable to run {} build: {err}", oci_backend.as_str())
                        })?;

                    if !status.success() {
                        anyhow::bail!(
                            "Unable to build cicada bootstrap image, please check the {} build output",
                            oci_backend.as_str()
                        );
                    }

                    info!("\nBuilt cicada bootstrap image: {}\n", tag.bold());

                    Some(tag)
                } else {
                    None
                };

                let pipeline = resolve_pipeline(pipeline)?;
                let pipeline_file_name = pipeline.file_name().unwrap();

                #[cfg(feature = "telemetry")]
                let telem_join = segment_enabled().then(|| {
                    let pipeline_name = pipeline_file_name.to_string_lossy().to_string();
                    let pipeline_length = std::fs::read_to_string(&pipeline)
                        .map(|f| f.lines().count())
                        .ok();

                    tokio::spawn(
                        TrackEvent::PipelineExecuted {
                            pipeline_name,
                            pipeline_length,
                        }
                        .post(),
                    )
                });

                let project_dir = pipeline.parent().unwrap().parent().unwrap();
                let pipeline_url = Url::from_file_path(&pipeline)
                    .map_err(|_| anyhow::anyhow!("Unable to convert pipeline path to URL"))?;

                let gh_repo = github_repo().await.ok().flatten();

                info!("Building pipeline: {}", pipeline.display().bold());

                let out = {
                    let tmp_file = tempfile::NamedTempFile::new()?;

                    run_deno_builder(
                        &deno_exe,
                        &LOCAL_CLI_SCRIPT,
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

                let pipeline = serde_json::from_str::<Pipeline>(&out)?;

                // Check if we should run this pipeline based on the git event
                if let (Ok(git_event), Ok(base_ref)) = (
                    std::env::var("CICADA_GIT_EVENT"),
                    std::env::var("CICADA_BASE_REF"),
                ) {
                    match pipeline.on {
                        Some(job::Trigger::Options { push, pull_request }) => match &*git_event {
                            "pull_request" if !pull_request.contains(&base_ref) => {
                                info!(
                                    "Skipping pipeline because branch {} is not in {}: {:?}",
                                    base_ref.bold(),
                                    "pull_request".bold(),
                                    pull_request
                                );
                                std::process::exit(2);
                            }
                            "push" if !push.contains(&base_ref) => {
                                info!(
                                    "Skipping pipeline because branch {} is not in {}: {:?}",
                                    base_ref.bold(),
                                    "push".bold(),
                                    push
                                );
                                std::process::exit(2);
                            }
                            _ => {}
                        },
                        Some(job::Trigger::DenoFunction) => {
                            anyhow::bail!("TypeScript trigger functions are unimplemented")
                        }
                        None => {}
                    }
                }

                let mut jobs = HashMap::from_iter(
                    pipeline
                        .jobs
                        .into_iter()
                        .enumerate()
                        .map(|(index, job)| (job.uuid, (index, job))),
                );

                let mut all_secrets: Vec<(String, String)> = vec![];

                // Look for the secret in the environment or error
                for secret in secret {
                    all_secrets.push((
                        secret.clone(),
                        std::env::var(&secret).with_context(|| {
                            format!("Could not find secret in environment: {secret}")
                        })?,
                    ));
                }

                if !no_dotenv {
                    // Load the .env file if it exists
                    let iter = match dotenv {
                        Some(path) => Some(dotenvy::from_path_iter(&path).with_context(|| {
                            format!("Could not load dotenv file: {}", path.display())
                        })?),
                        None => dotenvy::dotenv_iter().ok(),
                    };

                    if let Some(iter) = iter {
                        for (key, value) in iter.flatten() {
                            all_secrets.push((key, value));
                        }
                    }
                }

                // Load the secrets json file if it exists
                if let Some(path) = secrets_json {
                    let secrets: HashMap<String, String> =
                        serde_json::from_str(&std::fs::read_to_string(&path).with_context(
                            || format!("Could not load secrets json file: {}", path.display()),
                        )?)
                        .with_context(|| {
                            format!("Could not parse secrets json file: {}", path.display())
                        })?;

                    for (key, value) in secrets {
                        all_secrets.push((key, value));
                    }
                }

                let nodes: Vec<Node> = jobs
                    .values()
                    .map(|(_, job)| Node::new(job.uuid, job.depends_on.to_vec()))
                    .collect();
                let graph = topological_sort(&invert_graph(&nodes))?;

                let mut exit_code = 0;
                'run_groups: for run_group in graph {
                    match futures::future::try_join_all(run_group.into_iter().map(|job| {
                        let (job_index, job) = jobs.remove(&job).unwrap();

                        let span = info_span!("job", job_name = job.display_name(job_index));
                        let _enter = span.enter();

                        let gh_repo = gh_repo.clone();
                        let pipeline_file_name = pipeline_file_name.to_os_string();
                        let project_dir = project_dir.to_path_buf();
                        let all_secrets = all_secrets.clone();
                        let cicada_bin_tag = cicada_bin_tag.clone();

                        tokio::spawn(
                            async move {
                                let tag = format!("cicada-{}", job.image);

                                let mut child = if buildkit_expiremental {
                                    let mut buildctl = Command::new("buildctl")
                                        // .arg("--debug")
                                        .arg("build")
                                        .arg("--local")
                                        .arg(format!("local={}", project_dir.display()))
                                        .arg("--progress")
                                        .arg("plain")
                                        .env("BUILDKIT_HOST", "docker-container://buildkitd")
                                        .stdin(Stdio::piped())
                                        .stdout(Stdio::piped())
                                        .stderr(Stdio::piped())
                                        .spawn()?;

                                    let llb_vec = job.to_llb(
                                        pipeline_file_name.to_str().unwrap(),
                                        &gh_repo,
                                        job_index,
                                    );

                                    let mut stdin = buildctl.stdin.take().unwrap();
                                    stdin.write_all(&llb_vec).in_current_span().await?;
                                    stdin.shutdown().in_current_span().await?;

                                    buildctl
                                } else {
                                    let mut args: Vec<String> = vec![
                                        "buildx".into(),
                                        "build".into(),
                                        "-t".into(),
                                        tag,
                                        "--build-context".into(),
                                        format!("local={}", project_dir.to_str().unwrap()),
                                        "--progress".into(),
                                        "plain".into(),
                                    ];

                                    if let Some(cicada_bin_tag) = &cicada_bin_tag {
                                        args.extend([
                                            "--build-context".into(),
                                            format!("cicada-bin=docker-image://{cicada_bin_tag}"),
                                        ]);
                                    }

                                    for (key, _) in &all_secrets {
                                        args.extend(["--secret".into(), format!("id={key}")]);
                                    }

                                    args.push("-".into());

                                    let mut buildx_cmd = Command::new(oci_backend.as_str());
                                    buildx_cmd
                                        .args(args)
                                        .stdin(Stdio::piped())
                                        .stdout(Stdio::piped())
                                        .stderr(Stdio::piped())
                                        .envs(all_secrets);

                                    let mut buildx = buildx_cmd.spawn()?;

                                    let dockerfile = job.to_dockerfile(
                                        pipeline_file_name.to_str().unwrap(),
                                        &gh_repo,
                                        job_index,
                                        cicada_bin_tag.is_some(),
                                    );

                                    buildx
                                        .stdin
                                        .as_mut()
                                        .unwrap()
                                        .write_all(dockerfile.as_bytes())
                                        .in_current_span()
                                        .await?;

                                    buildx
                                        .stdin
                                        .take()
                                        .unwrap()
                                        .shutdown()
                                        .in_current_span()
                                        .await?;

                                    buildx
                                };

                                // Print the output as it comes in
                                let stdout = child.stdout.take().unwrap();
                                let stderr = child.stderr.take().unwrap();

                                // TODO: Make this into a function that takes a stream, a color, and a display name
                                let stdout_handle = tokio::spawn(
                                    async move {
                                        let mut buf_reader = BufReader::new(stdout);
                                        let mut line = String::new();
                                        loop {
                                            if let Err(err) = buf_reader
                                                .read_line(&mut line)
                                                .in_current_span()
                                                .await
                                            {
                                                error!("{err}");
                                                return;
                                            }
                                            if line.is_empty() {
                                                return;
                                            }
                                            info!("{line}");
                                            line.clear();
                                        }
                                    }
                                    .in_current_span(),
                                );

                                let stderr_handle = tokio::spawn(
                                    async move {
                                        let mut buf_reader = BufReader::new(stderr);
                                        let mut line = String::new();
                                        loop {
                                            if let Err(err) = buf_reader
                                                .read_line(&mut line)
                                                .in_current_span()
                                                .await
                                            {
                                                error!("{err}");
                                                return;
                                            }
                                            if line.is_empty() {
                                                return;
                                            }

                                            info!("{line}");
                                            line.clear();
                                        }
                                    }
                                    .in_current_span(),
                                );

                                let long_name = job.long_name(job_index);

                                stdout_handle.await.with_context(|| {
                                    format!("Failed to read stdout for {long_name}")
                                })?;
                                stderr_handle.await.with_context(|| {
                                    format!("Failed to read stderr for {long_name}")
                                })?;

                                let status =
                                    child.wait().in_current_span().await.with_context(|| {
                                        format!("Failed to wait for {long_name} to finish")
                                    })?;

                                anyhow::Ok((long_name, status, job))
                            }
                            .in_current_span(),
                        )
                    }))
                    .await
                    {
                        Ok(results) => {
                            for result in results {
                                match result {
                                    Ok((long_name, exit_status, job)) => match job.on_fail {
                                        Some(OnFail::Ignore) if !exit_status.success() => {
                                            warn!("{long_name} failed with status {exit_status} but was ignored");
                                        }
                                        Some(OnFail::Stop) | None if !exit_status.success() => {
                                            error!("Build failed for {long_name} with status {exit_status}");
                                            exit_code = 1;
                                            break 'run_groups;
                                        }
                                        _ => {
                                            info!("{long_name} finished with status {exit_status}");
                                        }
                                    },
                                    Err(err) => {
                                        error!("{err}");
                                        exit_code = 1;
                                        break 'run_groups;
                                    }
                                }
                            }
                        }
                        Err(e) => bail!(e),
                    }
                }

                #[cfg(feature = "telemetry")]
                if let Some(join) = telem_join {
                    join.await.ok();
                }

                if exit_code != 0 {
                    std::process::exit(exit_code)
                }
            }
            Commands::Step { workflow, step } => {
                run_deno(
                    &RUNNER_CLI_SCRIPT,
                    vec![workflow.to_string(), step.to_string()],
                )
                .await?;
            }
            Commands::Init { pipeline } => {
                #[cfg(feature = "self-update")]
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
                //         info!("Installing Deno extension for VSCode");
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

                let pipeline_name = pipeline.as_deref().unwrap_or("pipeline");

                let pipeline_path = cicada_dir.join(format!("{pipeline_name}.ts"));

                if pipeline_path.exists() {
                    error!("Pipeline already exists: {}", pipeline_path.display());
                    std::process::exit(1);
                }

                tokio::fs::write(&pipeline_path, &*DEFAULT_PIPELINE).await?;

                // Cache deno dependencies
                if let Ok(mut out) = Command::new("deno")
                    .args(["cache", "-q", pipeline_path.to_str().unwrap()])
                    .spawn()
                {
                    out.wait().await.ok();
                }

                info!(
                    "\n{} Initialized Cicada pipeline: {}\n{} Run it with: {}\n",
                    " ◥◣ ▲ ◢◤ ".fg_rgb::<145, 209, 249>(),
                    pipeline_name.bold(),
                    "  ◸ ▽ ◹  ".fg_rgb::<145, 209, 249>(),
                    format!("cicada run {pipeline_name}").bold(),
                );
            }
            Commands::New { pipeline } => {
                #[cfg(feature = "self-update")]
                check_for_update().await;

                // Check if cicada is initialized
                let Ok(dir) = resolve_cicada_dir() else {
                    error!(
                        "Cicada is not initialized in this directory. Run {} to initialize it.",
                        "cicada init".bold()
                    );
                    std::process::exit(1);
                };

                let pipeline_path = dir.join(format!("{pipeline}.ts"));

                if pipeline_path.exists() {
                    error!("Pipeline already exists: {}", pipeline_path.display());
                    std::process::exit(1);
                }

                tokio::fs::write(&pipeline_path, &*DEFAULT_PIPELINE).await?;

                info!(
                    "\n{} Initialized Cicada pipeline: {}\n{} Run it with: {}\n",
                    " ◥◣ ▲ ◢◤ ".fg_rgb::<145, 209, 249>(),
                    pipeline.bold(),
                    "  ◸ ▽ ◹  ".fg_rgb::<145, 209, 249>(),
                    format!("cicada run {pipeline}").bold(),
                );
            }
            #[cfg(feature = "self-update")]
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
                        info!("\nAlready up to date: {ver}");
                    }
                    self_update::Status::Updated(ver) => {
                        info!("\nUpdated to version {ver}");
                    }
                }
            }
            #[cfg(not(feature = "self-update"))]
            Commands::Update => {
                error!("self update is not enabled in this build");
                std::process::exit(1);
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
            Commands::Doctor { oci_backend } => {
                info!("Checking for common problems...");
                runtime_checks(&oci_backend).await;
                info!("\nAll checks passed!");
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
            Commands::Completions { .. } => "completions",
            Commands::Doctor { .. } => "doctor",
        }
    }
}

#[tokio::main]
async fn main() {
    #[cfg(feature = "telemetry")]
    let _sentry_guard = sentry_init();
    if let Err(err) = logging_init() {
        eprintln!("Failed to init logger: {err:#?}");
        std::process::exit(1);
    }

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

    if let Err(err) = res {
        if std::env::var_os("CICADA_DEBUG").is_some() {
            error!("{err:#?}");
        } else {
            error!("{err}");
        }
        std::process::exit(1);
    }
}
