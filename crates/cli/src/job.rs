use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use anyhow::Context;
use buildkit_rs::{reference::Reference, util::oci::OciBackend};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};
use tracing::{error, info, Instrument};

use crate::{bin_deps::DENO_VERSION, git::Github};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OnFail {
    Ignore,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheSharing {
    Shared,
    Private,
    Locked,
}

impl From<CacheSharing> for buildkit_rs::llb::CacheSharingMode {
    fn from(sharing: CacheSharing) -> Self {
        match sharing {
            CacheSharing::Shared => buildkit_rs::llb::CacheSharingMode::Shared,
            CacheSharing::Private => buildkit_rs::llb::CacheSharingMode::Private,
            CacheSharing::Locked => buildkit_rs::llb::CacheSharingMode::Locked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheDirectory {
    pub path: Utf8PathBuf,
    pub sharing: Option<CacheSharing>,
}

impl CacheDirectory {
    fn to_mount(&self, working_directory: &Utf8PathBuf) -> buildkit_rs::llb::Mount {
        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            working_directory.join(&self.path)
        };

        buildkit_rs::llb::Mount::cache(
            path.clone(),
            path,
            self.sharing.map(Into::into).unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Trigger {
    Options {
        #[serde(default)]
        push: Vec<String>,
        #[serde(default)]
        pull_request: Vec<String>,
    },
    DenoFunction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Shell {
    Sh,
    Bash,
    Args { args: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum StepRun {
    Command { command: String },
    Args { args: Vec<String> },
    DenoFunction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub run: StepRun,
    pub name: Option<String>,
    #[serde(default)]
    pub cache_directories: Vec<CacheDirectory>,
    pub ignore_cache: Option<bool>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    pub secrets: Vec<String>,
    pub working_directory: Option<Utf8PathBuf>,
    pub shell: Option<Shell>,
}

impl Step {
    fn to_exec<'a, 'b: 'a>(
        &'b self,
        root_mount: buildkit_rs::llb::Mount<'a>,
        parent_cache_directories: &'b [CacheDirectory],
        parent_working_directory: &'b Utf8PathBuf,
        env: &'b [String],
        job_index: usize,
        step_index: usize,
    ) -> buildkit_rs::llb::Exec<'a> {
        use buildkit_rs::llb::*;

        let mut exec = match &self.run {
            StepRun::Command { command } => match &self.shell {
                Some(Shell::Sh) | None => Exec::new(["/bin/sh", "-c", command]),
                Some(Shell::Bash) => Exec::new(["/bin/bash", "-c", command]),
                Some(Shell::Args { args }) => {
                    let mut args = args.clone();
                    args.push(command.clone());
                    Exec::new(args)
                }
            },
            StepRun::Args { args } => Exec::new(args.clone()),
            StepRun::DenoFunction => Exec::new([
                "cicada",
                "step",
                &job_index.to_string(),
                &step_index.to_string(),
            ]),
        }
        .with_mount(root_mount);

        // Custom name for the step
        exec = match (&self.name, &self.run) {
            (Some(name), StepRun::Command { command }) => {
                exec.with_custom_name(format!("{name} ({step_index}): {command}"))
            }
            (Some(name), StepRun::Args { args }) => {
                exec.with_custom_name(format!("{name} ({step_index}): {}", args.join(" ")))
            }
            (Some(name), StepRun::DenoFunction) => {
                exec.with_custom_name(format!("{name} ({step_index})"))
            }
            (None, StepRun::Command { command }) => exec.with_custom_name(command.clone()),
            (None, StepRun::Args { args }) => exec.with_custom_name(format!("{}", args.join(" "))),
            (None, StepRun::DenoFunction) => exec.with_custom_name(format!("Step {step_index}")),
        };

        // If the step has a working directory, we need to set it
        let working_directory = if let Some(working_directory) = &self.working_directory {
            // This is relative to the parent working directory if it is not absolute

            if working_directory.is_absolute() {
                working_directory.clone()
            } else {
                parent_working_directory.join(working_directory)
            }
        } else {
            parent_working_directory.clone()
        };

        exec = exec.with_cwd(working_directory.clone().into());

        for cache_directory in &self.cache_directories {
            exec = exec.with_mount(cache_directory.to_mount(&working_directory));
        }

        for cache_directory in parent_cache_directories {
            exec = exec.with_mount(cache_directory.to_mount(&working_directory));
        }

        // Cache the deno directory
        if StepRun::DenoFunction == self.run {
            exec = exec.with_mount(Mount::cache(
                "/root/.cache/deno",
                "/root/.cache/deno",
                CacheSharingMode::default(),
            ));
        }

        for secret in &self.secrets {
            let dest = Utf8PathBuf::from("/run/secrets").join(secret);
            exec = exec.with_mount(Mount::secret(dest, secret, 0, 0, 0o600, false));
        }

        // Set the environment variables
        exec = exec.with_env(
            self.env
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .chain(env.iter().cloned())
                .collect(),
        );

        // Invalidate the cache if the step is marked as ignore_cache by generating a non-deterministic environment variable
        if self.ignore_cache.unwrap_or(false) {
            exec = exec.ignore_cache(true);
        }

        exec
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub uuid: uuid::Uuid,
    pub image: String,
    pub steps: Vec<Step>,
    pub name: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cache_directories: Vec<CacheDirectory>,
    pub working_directory: Option<Utf8PathBuf>,
    #[serde(default)]
    pub depends_on: Vec<uuid::Uuid>,
    pub on_fail: Option<OnFail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InspectInfo {
    #[serde(default)]
    config: InspectConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InspectConfig {
    pub hostname: Option<String>,
    pub domainname: Option<String>,
    pub user: Option<String>,
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct JobResolved {
    pub job: Box<Job>,
    pub image_info: Box<InspectInfo>,
    pub image_reference: Reference,
}

impl JobResolved {
    pub fn to_llb(
        &self,
        module_name: impl AsRef<str>,
        project_directory: impl AsRef<Path>,
        github: &Option<Github>,
        job_index: usize,
        cicada_image: Option<impl Into<String>>,
    ) -> Vec<u8> {
        use buildkit_rs::llb::*;

        let working_directory = self
            .job
            .working_directory
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("/app"));

        let mut local: Local = Local::new("local".into());

        // Try to load excludes from `.cicadaignore`, `.containerignore`, `.dockerignore` in that order
        for ignore_name in &[".cicadaignore", ".containerignore", ".dockerignore"] {
            let ignore_path = project_directory.as_ref().join(ignore_name);
            if ignore_path.is_file() {
                // Read the file, strip comments and empty lines
                let ignore_file = match std::fs::File::open(ignore_path) {
                    Ok(ignore_file) => ignore_file,
                    Err(err) => {
                        error!(%err, "Failed to open ignore file {ignore_name}: {err}");
                        break;
                    }
                };

                let list = match buildkit_rs::ignore::read_ignore_to_list(ignore_file) {
                    Ok(list) => list,
                    Err(err) => {
                        error!(%err, "Failed to read ignore file {ignore_name}: {err}");
                        break;
                    }
                };

                local = local.with_excludes(list);

                break;
            }
        }

        let image = Image::reference(self.image_reference.clone())
            .with_platform(Platform::LINUX_AMD64)
            .with_custom_name(self.job.name.clone().unwrap())
            .with_resolve_mode(ResolveMode::Local);

        let deno_image = Image::new(format!("docker.io/denoland/deno:bin-{DENO_VERSION}"))
            .with_platform(Platform::LINUX_AMD64);

        let deno_mount = Mount::layer_readonly(deno_image.output(), "/usr/local/bin/deno")
            .with_selector("/deno");

        let cicada_image = match cicada_image {
            Some(cicada_image) => Image::local(cicada_image.into()),
            None => Image::new(format!(
                "docker.io/cicadahq/cicada-bin:{}",
                env!("CARGO_PKG_VERSION")
            )),
        }
        .with_platform(Platform::LINUX_AMD64);

        let cicada_mount = Mount::layer_readonly(cicada_image.output(), "/usr/local/bin/cicada")
            .with_selector("/cicada");

        let local_cp = Exec::shell(
            "/bin/sh",
            format!("mkdir -p {working_directory} && cp -r /local/. {working_directory}"),
        )
        .with_mount(Mount::layer(image.output(), "/", 0))
        .with_mount(Mount::layer_readonly(local.output(), "/local"))
        .with_custom_name("Copy local files");

        let mut env = self.image_info.config.env.clone();

        env.extend([
            "CI=1".into(),
            format!(
                "CICADA_PIPELINE_FILE={working_directory}/.cicada/{}",
                module_name.as_ref()
            ),
            "CICADA_JOB=1".into(),
        ]);

        if let Some(github_repository) = github {
            env.push(format!("GITHUB_REPOSITORY={github_repository}"));
        }

        env.extend(self.job.env.iter().map(|(k, v)| format!("{k}={v}")));

        let mut prev_step = Arc::new(local_cp);
        for (step_index, step) in self.job.steps.iter().enumerate() {
            let output = MultiOwnedOutput::output(&prev_step, 0);
            let root = Mount::layer(output, "/", 0);

            let exec = Arc::new(
                step.to_exec(
                    root,
                    &self.job.cache_directories,
                    &working_directory,
                    &env,
                    job_index,
                    step_index,
                )
                .with_mount(deno_mount.clone())
                .with_mount(cicada_mount.clone()),
            );

            prev_step = exec;
        }

        let bytes = Definition::new(prev_step.output(0)).into_bytes();

        bytes
    }

    pub async fn solve(
        self,
        job_index: usize,
        github: Option<Github>,
        pipeline_name: String,
        project_directory: String,
        all_secrets: Vec<(String, String)>,
        cicada_image: Option<String>,
        buildctl_exe: PathBuf,
        no_cache: bool,
        oci_backend: OciBackend,
    ) -> anyhow::Result<(String, ExitStatus, Self)> {
        let name: String = self.job.name.clone().unwrap().replace('\"', "\"\"");

        let config = oci_spec::image::ConfigBuilder::default()
            // .user("root".to_string())
            // .working_dir(job.working_directory.clone())
            // .env(["ABC=123".to_owned()])
            // .cmd(["/bin/bash".to_oswned()])
            .entrypoint(["/app/hello-world".to_owned()])
            .build()
            .unwrap();

        let image_config = oci_spec::image::ImageConfigurationBuilder::default()
            .config(config)
            .build()
            .unwrap();

        let image_config_json = serde_json::to_string(&image_config)
            .context("Unable to serialize OCI spec to JSON")?
            .replace("\"", "\"\"");

        let mut buildctl = Command::new(&buildctl_exe);
        buildctl
            .arg("build")
            .arg("--local")
            .arg(format!("local={project_directory}"))
            .arg("--progress")
            .arg("plain")
            .arg("--output")
            .arg(format!(
                "type=docker,\"name={name}\",\"containerimage.config={image_config_json}\""
            ))
            .env(
                "BUILDKIT_HOST",
                format!("{}-container://cicada-buildkitd", oci_backend.as_str()),
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

        let llb_vec = self.to_llb(
            pipeline_name,
            &project_directory,
            &github,
            job_index,
            cicada_image,
        );

        let mut stdin = buildctl_child.stdin.take().unwrap();
        stdin.write_all(&llb_vec).in_current_span().await?;
        stdin.shutdown().in_current_span().await?;
        drop(stdin);

        // Print the output as it comes in
        let stderr = buildctl_child.stderr.take().unwrap();

        let stderr_handle = tokio::spawn(
            async move {
                let mut buf_reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    if let Err(err) = buf_reader.read_line(&mut line).in_current_span().await {
                        error!("{err}");
                        return;
                    }
                    if line.is_empty() {
                        return;
                    }

                    info!("{}", line.trim_end_matches('\n'));
                    line.clear();
                }
            }
            .in_current_span(),
        );

        let long_name = self.long_name(job_index);

        // Stdout is the tar stream that we want to pipe to docker load
        let mut stdout = buildctl_child.stdout.take().unwrap();

        let mut docker_load = Command::new("docker")
            .arg("load")
            .stdin(Stdio::piped())
            .spawn()?;

        let mut docker_load_stdin = docker_load.stdin.take().unwrap();
        tokio::io::copy(&mut stdout, &mut docker_load_stdin)
            .in_current_span()
            .await?;
        drop(docker_load_stdin);

        stderr_handle
            .in_current_span()
            .await
            .with_context(|| format!("Failed to read stderr for {long_name}"))?;

        let docker_load_status = docker_load
            .wait()
            .in_current_span()
            .await
            .with_context(|| format!("Failed to wait for docker load to finish"))?;

        if !docker_load_status.success() {
            return Err(anyhow::anyhow!(
                "Failed to load image for {long_name} into docker"
            ));
        }

        let status = buildctl_child
            .wait()
            .in_current_span()
            .await
            .with_context(|| format!("Failed to wait for {long_name} to finish"))?;

        anyhow::Ok((long_name, status, self))
    }

    pub fn display_name(&self, index: usize) -> String {
        self.job
            .name
            .clone()
            .unwrap_or_else(|| format!("{}-{index}", self.job.image))
    }

    pub fn long_name(&self, index: usize) -> String {
        let image = &self.job.image;
        match &self.job.name {
            Some(name) => format!("{name} ({image}-{index})"),
            None => format!("{image}-{index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    pub jobs: Vec<Job>,
    pub on: Option<Trigger>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum CicadaType {
    Pipeline(Pipeline),
    Image(Job),
}
