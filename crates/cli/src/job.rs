use std::{collections::HashMap, sync::Arc};

use buildkit_rs::llb::{MultiBorrowedOutput, MultiOwnedOutput, OpMetadataBuilder};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::{deno::DENO_VERSION, git::Github};

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

impl CacheSharing {
    fn as_text(&self) -> &'static str {
        match self {
            CacheSharing::Shared => "shared",
            CacheSharing::Private => "private",
            CacheSharing::Locked => "locked",
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
    fn to_docker_flag(&self, working_directory: &Utf8PathBuf) -> String {
        let path = if self.path.is_absolute() {
            self.path.to_owned()
        } else {
            working_directory.join(&self.path)
        };

        let mut flag = format!("--mount=type=cache,target={path}");

        if let Some(sharing) = self.sharing {
            flag.push_str(&format!(",sharing={}", sharing.as_text()));
        }

        flag
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
pub enum StepRun {
    Command { command: String },
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
}

impl Step {
    fn to_dockerfile_lines(
        &self,
        parent_cache_directories: &[CacheDirectory],
        parent_working_directory: &Utf8PathBuf,
        job_index: usize,
        step_index: usize,
    ) -> Vec<String> {
        let mut lines: Vec<String> = vec![];

        // If the step has a working directory, we need to set it
        let working_directory = if let Some(working_directory) = &self.working_directory {
            // This is relative to the parent working directory if it is not absolute
            let working_directory = if working_directory.is_absolute() {
                working_directory.to_owned()
            } else {
                parent_working_directory.join(working_directory)
            };

            lines.push(format!("WORKDIR {working_directory}"));

            working_directory
        } else {
            parent_working_directory.to_owned()
        };

        let mut run_cmd_parts: Vec<String> = vec!["RUN".into()];

        for cache_directory in &self.cache_directories {
            run_cmd_parts.push(cache_directory.to_docker_flag(&working_directory));
        }

        for cache_directory in parent_cache_directories {
            run_cmd_parts.push(cache_directory.to_docker_flag(&working_directory));
        }

        // Cache the deno directory
        if StepRun::DenoFunction == self.run {
            run_cmd_parts.push("--mount=type=cache,target=/root/.cache/deno".into());
        }

        for secret in &self.secrets {
            run_cmd_parts.push(format!("--mount=type=secret,id={secret}"));
        }

        // Set the environment variables
        for (key, value) in &self.env {
            run_cmd_parts.push(format!("{}={}", shlex::quote(key), shlex::quote(value)));
        }

        // Invalidate the cache if the step is marked as ignore_cache by generating a non-deterministic environment variable
        if self.ignore_cache.unwrap_or(false) {
            run_cmd_parts.push(format!("CICADA_CACHE_BUST={}", uuid::Uuid::new_v4()));
        }

        match &self.run {
            StepRun::Command { command } => {
                run_cmd_parts.push(command.into());
            }
            StepRun::DenoFunction => {
                run_cmd_parts.push(format!("cicada step {job_index} {step_index}"));
            }
        }

        lines.push(run_cmd_parts.join(" "));

        // Restore the working directory if it was changed
        if self.working_directory.is_some() {
            lines.push(format!("WORKDIR {parent_working_directory}"));
        }

        lines
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

impl Job {
    /// This converts the job into a dockerfile definition, the plan is to convert this into
    /// a direct llb definition in the future
    pub fn to_dockerfile(
        &self,
        module_name: &str,
        github: &Option<Github>,
        job_index: usize,
        bootstrap: bool,
    ) -> String {
        let mut lines: Vec<String> = vec!["# syntax = docker/dockerfile:1.4".into()];

        lines.push(format!(
            "FROM docker.io/denoland/deno:bin-{DENO_VERSION} as deno-bin"
        ));
        if !bootstrap {
            lines.push(format!(
                "FROM --platform=linux/amd64 docker.io/cicadahq/cicada-bin:{} as cicada-bin",
                env!("CARGO_PKG_VERSION")
            ));
        }

        lines.push(format!("FROM --platform=linux/amd64 {}", self.image));
        lines.push("ENV CI=true".into());

        lines.push("COPY --from=cicada-bin /cicada /usr/local/bin/cicada".into());
        lines.push("COPY --from=deno-bin /deno /usr/local/bin/deno".into());

        // Make working directory
        let working_directory = self
            .working_directory
            .clone()
            .unwrap_or_else(|| Utf8PathBuf::from("/app"));

        lines.push(format!("RUN mkdir -p {working_directory}"));
        lines.push("RUN mkdir -p /workspace".into());

        lines.push(format!("COPY --from=local . {working_directory}"));

        lines.push(format!("WORKDIR {working_directory}"));

        // Set the env for the steps
        for (key, value) in &self.env {
            lines.push(format!("ENV {}={}", shlex::quote(key), shlex::quote(value)));
        }

        lines.push("ENV CICADA_JOB=1".into());
        lines.push(format!(
            "ENV CICADA_PIPELINE_FILE={working_directory}/.cicada/{module_name}",
        ));
        if let Some(github_repository) = github {
            lines.push(format!("ENV GITHUB_REPOSITORY={github_repository}"));
        }

        // Run the steps
        for (step_index, step) in self.steps.iter().enumerate() {
            lines.extend(step.to_dockerfile_lines(
                &self.cache_directories,
                &working_directory,
                job_index,
                step_index,
            ));
        }

        lines.join("\n")
    }

    pub fn to_llb(&self) -> Vec<u8> {
        use buildkit_rs::llb::SingleBorrowedOutput;

        let image =
            buildkit_rs::llb::Image::new(&self.image).with_custom_name(self.name.clone().unwrap());

        let deno_image =
            buildkit_rs::llb::Image::new(format!("docker.io/denoland/deno:bin-{DENO_VERSION}"));

        let local = buildkit_rs::llb::Local::new("local".into());

        let cicada_image = buildkit_rs::llb::Image::new(format!(
            "docker.io/cicadahq/cicada-bin:{}",
            env!("CARGO_PKG_VERSION")
        ));

        let deno_cp = buildkit_rs::llb::Exec::shlex("cp /deno/deno /usr/local/bin/deno")
            .with_mount(buildkit_rs::llb::Mount::layer(image.output(), "/", 0))
            .with_mount(buildkit_rs::llb::Mount::layer_readonly(
                deno_image.output(),
                "/deno",
            ));

        let cicada_cp = buildkit_rs::llb::Exec::shlex("cp /cicada/cicada /usr/local/bin/cicada")
            .with_mount(buildkit_rs::llb::Mount::layer(deno_cp.output(0), "/", 0))
            .with_mount(buildkit_rs::llb::Mount::layer_readonly(
                cicada_image.output(),
                "/cicada",
            ));

        let local_cp =
            buildkit_rs::llb::Exec::shlex("/bin/sh -c 'mkdir -p /app && cp -r /local/* /app'")
                .with_mount(buildkit_rs::llb::Mount::layer(cicada_cp.output(0), "/", 0))
                .with_mount(buildkit_rs::llb::Mount::layer_readonly(
                    local.output(),
                    "/local",
                ));

        let cicada_version = buildkit_rs::llb::Exec::shlex("cicada -V")
            .with_mount(buildkit_rs::llb::Mount::layer(local_cp.output(0), "/", 0));

        let mut prev_step = Arc::new(cicada_version);
        for step in &self.steps {
            match &step.run {
                StepRun::Command { command } => {
                    let output = MultiOwnedOutput::output(&prev_step, 0);

                    let exec = Arc::new(
                        buildkit_rs::llb::Exec::shlex(format!("/bin/bash -c '{command}'"))
                            .with_custom_name(command)
                            .with_mount(buildkit_rs::llb::Mount::layer(output, "/", 0))
                            .with_env(
                                vec![
                                    "PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
                                    "RUSTUP_HOME=/usr/local/rustup".into(),
                                    "CARGO_HOME=/usr/local/cargo".into(),
                                    "RUST_VERSION=1.68.2".into()
                                ]
                            )
                            .with_cwd("/app".into())
                    );

                    prev_step = exec;
                }
                StepRun::DenoFunction => {}
            }
        }

        let bytes = buildkit_rs::llb::Definition::new(prev_step.output(0)).into_bytes();

        bytes
    }

    pub fn display_name(&self, index: usize) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("{}-{index}", self.image))
    }

    pub fn long_name(&self, index: usize) -> String {
        let image = &self.image;
        match &self.name {
            Some(name) => format!("{name} ({image}-{index})"),
            None => format!("{image}-{index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pipeline {
    pub jobs: Vec<Job>,
    pub on: Option<Trigger>,
}
