mod bin_deps;
mod dag;
mod debug;
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
use buildkit_rs::util::oci::OciBackend;
use clap_complete::generate;
use dialoguer::theme::ColorfulTheme;
use logging::logging_init;
use oci::OciArgs;
use once_cell::sync::Lazy;
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{ExitCode, Stdio},
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
    bin_deps::{buildctl_exe, deno_exe},
    dag::{invert_graph, topological_sort, Node},
    git::github_repo,
    job::{InspectInfo, JobResolved, OnFail, Pipeline},
};

// Transform from https://deno.land/x/cicada/mod.ts to https://deno.land/x/cicada@vX.Y.X/mod.ts
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

static TEMPLATES: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        replace_with_version(include_str!("../scripts/template-default.ts")),
        replace_with_version(include_str!("../scripts/template-node.ts")),
        replace_with_version(include_str!("../scripts/template-rust.ts")),
    ]
});

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
            error!("Cicada requires Docker to run. Please install it using one of the methods or install it from https://docs.docker.com/engine/install");
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

        #[command(flatten)]
        oci_args: OciArgs,

        /// Disable caching
        #[arg(long)]
        no_cache: bool,
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
    Open {
        /// Pipeline to open
        pipeline: PathBuf,
    },
    #[command(hide = true)]
    Doctor {
        #[command(flatten)]
        oci_args: OciArgs,
    },
    #[command(subcommand, hide = true)]
    Debug(debug::DebugCommand),
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
                oci_args,
                no_cache,
            } => {
                let oci_backend = oci_args.oci_backend();

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
                let buildctl_exe = buildctl_exe().await?;

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

                let project_directory = pipeline.parent().unwrap().parent().unwrap();
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
                        project_directory,
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

                info!(trigger = true);

                let inspect_output = Command::new(oci_backend.as_str())
                    .args([
                        "inspect",
                        "cicada-buildkitd",
                        "--type",
                        "container",
                        "--format",
                        "{{json .}}",
                    ])
                    .output()
                    .await?;

                if !inspect_output.status.success() {
                    Command::new(oci_backend.as_str())
                        .args([
                            "run",
                            "-d",
                            "--name",
                            "cicada-buildkitd",
                            "--privileged",
                            "moby/buildkit:latest",
                        ])
                        .status()
                        .await?;
                } else {
                    let containers: serde_json::Value =
                        serde_json::from_slice(&inspect_output.stdout)?;

                    if containers["State"]["Status"] != "running" {
                        Command::new(oci_backend.as_str())
                            .args(["start", "cicada-buildkitd"])
                            .status()
                            .await?;
                    }
                }

                // Populate the jobs with `docker inspect` data
                let mut populated_jobs: Vec<JobResolved> = vec![];
                for job in pipeline.jobs {
                    let resolved_image_name = buildkit_rs::reference::Reference::parse(&job.image)
                        .with_context(|| {
                            format!(
                                "Unable to parse image name: {}",
                                job.image.to_string().bold()
                            )
                        })?;

                    info!("Pulling image: {}", resolved_image_name.to_string().bold());

                    // Run pull to grab the image
                    let mut pull_child = Command::new(oci_backend.as_str())
                        .args([
                            "pull",
                            &resolved_image_name.to_string(),
                            "--platform",
                            "linux/amd64",
                        ])
                        .spawn()?;

                    if !pull_child.wait().await?.success() {
                        anyhow::bail!(
                            "Unable to pull image: {}",
                            resolved_image_name.to_string().bold()
                        );
                    }

                    // Run inspect to grab the image info
                    let docker_inspect_output = Command::new(oci_backend.as_str())
                        .args([
                            "inspect",
                            &resolved_image_name.to_string(),
                            "--type",
                            "image",
                            "--format",
                            "{{json .}}",
                        ])
                        .output()
                        .await?;

                    if !docker_inspect_output.status.success() {
                        anyhow::bail!(
                            "Unable to inspect image: {}",
                            resolved_image_name.to_string().bold()
                        );
                    }

                    // Deserialize the image info
                    let image_info: InspectInfo =
                        serde_json::from_slice(&docker_inspect_output.stdout)
                            .context("Unable to deserialize image info")?;

                    populated_jobs.push(JobResolved {
                        job: Box::new(job),
                        image_info: Box::new(image_info),
                    });

                    eprintln!();
                }

                let mut jobs = HashMap::from_iter(
                    populated_jobs
                        .into_iter()
                        .enumerate()
                        .map(|(index, job)| (job.job.uuid, (index, job))),
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
                    .map(|(_, job)| Node::new(job.job.uuid, job.job.depends_on.to_vec()))
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
                        let project_directory = project_directory.to_path_buf();
                        let all_secrets = all_secrets.clone();
                        // TODO: Add back bootstrapping the local cicada image
                        let _cicada_bin_tag = cicada_bin_tag.clone();
                        let buildctl_exe = buildctl_exe.clone();

                        tokio::spawn(
                            async move {
                                let mut buildctl = Command::new(buildctl_exe);
                                buildctl
                                    .arg("build")
                                    .arg("--local")
                                    .arg(format!("local={}", project_directory.display()))
                                    .arg("--progress")
                                    .arg("plain")
                                    .env(
                                        "BUILDKIT_HOST",
                                        format!(
                                            "{}-container://cicada-buildkitd",
                                            oci_backend.as_str()
                                        ),
                                    );

                                if no_cache {
                                    buildctl.arg("--no-cache");
                                }

                                for (key, _) in &all_secrets {
                                    buildctl.arg("--secret").arg(format!("id={key}"));
                                }

                                let mut buildctl_child = buildctl
                                    .envs(all_secrets)
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::piped())
                                    .stderr(Stdio::piped())
                                    .spawn()?;

                                let llb_vec = job.to_llb(
                                    pipeline_file_name.to_str().unwrap(),
                                    &project_directory,
                                    &gh_repo,
                                    job_index,
                                );

                                let mut stdin = buildctl_child.stdin.take().unwrap();
                                stdin.write_all(&llb_vec).in_current_span().await?;
                                stdin.shutdown().in_current_span().await?;

                                // Print the output as it comes in
                                let stdout = buildctl_child.stdout.take().unwrap();
                                let stderr = buildctl_child.stderr.take().unwrap();

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
                                    buildctl_child.wait().in_current_span().await.with_context(
                                        || format!("Failed to wait for {long_name} to finish"),
                                    )?;

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
                                    Ok((long_name, exit_status, job)) => match job.job.on_fail {
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

                let cicada_dir = PathBuf::from(".cicada");

                if cicada_dir.exists() {
                    anyhow::bail!(
                        "Cicada directory already exists, run {} to create a new pipeline",
                        "cicada new".bold()
                    );
                } else {
                    std::fs::create_dir(&cicada_dir)?;
                }

                let pipeline_name = match pipeline {
                    Some(pipeline) => pipeline,
                    None => dialoguer::Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("What should we call your pipeline")
                        .interact_text()?,
                }
                .replace(['\\', '/', '.', ' '], "-");

                let pipeline_path = cicada_dir.join(format!("{pipeline_name}.ts"));

                if pipeline_path.exists() {
                    error!("Pipeline already exists: {}", pipeline_path.display());
                    std::process::exit(1);
                }

                tokio::fs::write(
                    &pipeline_path,
                    &*TEMPLATES[dialoguer::Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Select a template")
                        .default(0)
                        // TODO: add more templates, contribs welcome :D
                        .items(&["Default", "Node", "Rust"])
                        .interact()?],
                )
                .await?;

                if std::env::var("TERM_PROGRAM").as_deref() == Ok("vscode") {
                    let bin_name = match std::env::var("TERM_PROGRAM_VERSION") {
                        Ok(version) if version.contains("insider") => "code-insiders",
                        _ => "code",
                    };

                    let should_install = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Would you like to setup autocomplete for VSCode?")
                        .default(true)
                        .interact()?;

                    if should_install {
                        // Check if deno extension is installed
                        let deno_extension_installed = String::from_utf8_lossy(
                            &Command::new(bin_name)
                                .args(["--list-extensions"])
                                .output()
                                .await?
                                .stdout,
                        )
                        .contains("denoland.vscode-deno");

                        if !deno_extension_installed {
                            info!("Installing Deno extension for VSCode");
                            Command::new(bin_name)
                                .args(["--install-extension", "denoland.vscode-deno"])
                                .spawn()
                                .unwrap()
                                .wait()
                                .await?;
                        }

                        // Check for the .vscode/settings.json file
                        let settings_path = PathBuf::from(".vscode/settings.json");
                        if !settings_path.exists() {
                            info!("Creating VSCode settings file");
                            std::fs::create_dir_all(".vscode")?;
                            tokio::fs::write(
                                &settings_path,
                                "{\"\n  deno.enablePaths\": [\".cicada\"]\n}",
                            )
                            .await?;
                        } else {
                            info!("Add the following to your VSCode settings file: \"deno.enablePaths\": [\".cicada\"]");
                        }
                    }
                }

                // Cache deno dependencies
                if let Ok(mut out) = Command::new("deno")
                    .args(["cache", "-q", pipeline_path.to_str().unwrap()])
                    .spawn()
                {
                    out.wait().await.ok();
                }

                info!(
                    "\n{} Initialized Cicada pipeline: {}\n{} Run it with: {}\n ",
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
                    anyhow::bail!(
                        "Cicada is not initialized in this directory. Run {} to initialize it.",
                        "cicada init".bold()
                    );
                };

                let pipeline_path = dir.join(format!("{pipeline}.ts"));

                if pipeline_path.exists() {
                    anyhow::bail!("Pipeline already exists: {}", pipeline_path.display());
                }

                tokio::fs::write(&pipeline_path, &*TEMPLATES[0]).await?;

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
                anyhow::bail!("self update is not enabled in this build");
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
            Commands::Open { pipeline } => {
                let resolved_pipeline = resolve_pipeline(pipeline)?;
                match std::env::var("EDITOR") {
                    Ok(editor) => open::with(resolved_pipeline, editor)?,
                    Err(_) => open::that(resolved_pipeline)?,
                }
            }
            Commands::Doctor { oci_args } => {
                info!("Checking for common problems...");
                runtime_checks(&oci_args.oci_backend()).await;
                info!("\nAll checks passed!");
            }
            Commands::Debug(debug_command) => debug_command.run().await?,
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
            Commands::Open { .. } => "open",
            Commands::Doctor { .. } => "doctor",
            Commands::Debug { .. } => "debug",
        }
    }

    #[cfg(feature = "telemetry")]
    fn track(&self) -> bool {
        match self {
            Commands::Run { .. } => true,
            Commands::Step { .. } => false,
            Commands::Init { .. } => true,
            Commands::New { .. } => true,
            Commands::Update => true,
            Commands::Completions { .. } => false,
            Commands::Open { .. } => false,
            Commands::Doctor { .. } => true,
            Commands::Debug { .. } => false,
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    #[cfg(feature = "telemetry")]
    let _sentry_guard = sentry_init();
    if let Err(err) = logging_init() {
        eprintln!("Failed to init logger: {err:#?}");
        return ExitCode::FAILURE;
    }

    if std::env::var_os("CICADA_FORCE_COLOR").is_some() {
        owo_colors::set_override(true);
    }

    let command = Commands::parse();

    #[cfg(feature = "telemetry")]
    let telem_join = (command.track() && segment_enabled()).then(|| {
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

    match res {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            if std::env::var_os("CICADA_DEBUG").is_some() {
                error!("{err:#?}");
            } else {
                error!("{err}");
            }
            ExitCode::FAILURE
        }
    }
}
