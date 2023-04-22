use std::fs::read_to_string;

use anyhow::Context;
use anyhow::Result;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::util::data_path;
use crate::util::digest;

use super::SEGMENT_WRITE_KEY;

static ANONYMOUS_ID: Lazy<Option<String>> = Lazy::new(|| {
    let data_path = data_path().ok()?.join("segment_anonymous_id");

    let contents = match read_to_string(&data_path) {
        Ok(contents) => contents.trim().to_string(),
        Err(_) => {
            let uuid = Uuid::new_v4().to_string();
            std::fs::write(&data_path, &uuid).ok()?;
            uuid
        }
    };

    Some(contents)
});

static SEGMENT_SALT: Lazy<Option<String>> = Lazy::new(|| {
    let data_path = data_path().ok()?.join("segment_salt");

    let contents = match read_to_string(&data_path) {
        Ok(contents) => contents.trim().to_string(),
        Err(_) => {
            let uuid = Uuid::new_v4().to_string();
            std::fs::write(&data_path, &uuid).ok()?;
            uuid
        }
    };

    Some(contents)
});

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum TrackEvent {
    SubcommandExecuted {
        subcommand_name: String,
    },
    PipelineExecuted {
        pipeline_name: String,
        pipeline_length: Option<usize>,
    },
}

impl TrackEvent {
    fn name(&self) -> &'static str {
        match self {
            TrackEvent::SubcommandExecuted { .. } => "Subcommand Executed",
            TrackEvent::PipelineExecuted { .. } => "Pipeline Executed",
        }
    }

    pub async fn post(self) -> Result<reqwest::Response> {
        let segment_write_key = SEGMENT_WRITE_KEY.context("No segment write key found")?;

        let anonymous_id = (*ANONYMOUS_ID)
            .to_owned()
            .context("failed to acquire user id")?;

        let event_name = self.name().to_owned();

        let mut properties: Map<String, Value> = match self {
            TrackEvent::SubcommandExecuted { subcommand_name } => {
                [("subcommand_name".to_owned(), Value::String(subcommand_name))]
                    .into_iter()
                    .collect()
            }
            TrackEvent::PipelineExecuted {
                pipeline_name,
                pipeline_length,
            } => [
                (
                    "pipeline_name_hash".into(),
                    digest(
                        format!(
                            "{}{pipeline_name}",
                            (*SEGMENT_SALT)
                                .to_owned()
                                .context("failed to acquire salt")?
                        )
                        .as_bytes(),
                    )
                    .into(),
                ),
                ("pipeline_length".into(), pipeline_length.into()),
                (
                    "gh_actions".into(),
                    std::env::var_os("GITHUB_ACTIONS").is_some().into(),
                ),
                ("vercel".into(), std::env::var_os("VERCEL").is_some().into()),
                (
                    "circle_ci".into(),
                    std::env::var_os("CIRCLECI").is_some().into(),
                ),
                (
                    "gitlab".into(),
                    std::env::var_os("GITLAB_CI").is_some().into(),
                ),
                ("travis".into(), std::env::var_os("TRAVIS").is_some().into()),
                (
                    "jenkins".into(),
                    std::env::var_os("JENKINS_URL").is_some().into(),
                ),
                (
                    "azure".into(),
                    std::env::var_os("BUILD_BUILDURI").is_some().into(),
                ),
            ]
            .into_iter()
            .collect(),
        };

        // Insert the default properties (os, architecture, environment, cli_version)
        //
        // Want to make sure the telemetry is useful but not too identifying basically
        // just identify which binary build is being used and nothing about the user's
        // env or machine besides that
        properties.insert("os".to_owned(), std::env::consts::OS.into());
        properties.insert("architecture".to_owned(), std::env::consts::ARCH.into());

        #[cfg(target_env = "gnu")]
        properties.insert("environment".to_owned(), "gnu".into());
        #[cfg(target_env = "musl")]
        properties.insert("environment".to_owned(), "musl".into());
        #[cfg(not(any(target_env = "gnu", target_env = "musl")))]
        properties.insert("environment".to_owned(), Value::Null);

        properties.insert("cli_version".to_owned(), env!("CARGO_PKG_VERSION").into());

        let timestamp = OffsetDateTime::now_utc();

        reqwest::Client::new()
            .post("https://api.segment.io/v1/track")
            .basic_auth::<_, &str>(segment_write_key, None)
            .json(&Track {
                anonymous_id,
                event: event_name,
                properties,
                timestamp,
            })
            .send()
            .await?
            .error_for_status()
            .context("failed to post track event")
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Track {
    anonymous_id: String,
    event: String,
    properties: Map<String, Value>,
    timestamp: OffsetDateTime,
}
