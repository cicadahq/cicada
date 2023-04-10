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

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum TrackEvent {
    SubcommandExecuted { subcommand_name: String },
}

impl TrackEvent {
    pub async fn post(self) -> Result<reqwest::Response> {
        let segment_write_key =
            std::env::var("SEGMENT_WRITE_KEY").context("No segment write key found")?;

        let anonymous_id = (*ANONYMOUS_ID)
            .to_owned()
            .context("failed to acquire user id")?;

        let (event, properties) = match self {
            TrackEvent::SubcommandExecuted { subcommand_name } => (
                "subcommand_executed".into(),
                [("subcommand_name".to_owned(), Value::String(subcommand_name))]
                    .into_iter()
                    .collect(),
            ),
        };

        let timestamp = OffsetDateTime::now_utc();

        reqwest::Client::new()
            .post("https://api.segment.io/v1/track")
            .basic_auth::<_, &str>(segment_write_key, None)
            .json(&Track {
                anonymous_id,
                event,
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
