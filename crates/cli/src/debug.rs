use std::fmt::Debug;

use buildkit_rs::{
    client::{random_id, session::secret::SecretSource, Client, SessionOptions, SolveOptions},
    proto::moby::buildkit::v1::{StatusResponse, Vertex, VertexLog, VertexWarning},
    util::oci::OciBackend,
};
use futures::StreamExt;
use humansize::{format_size, DECIMAL};
use owo_colors::OwoColorize;
use tracing::{error, info, warn};

use crate::oci::OciArgs;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum DebugCommand {
    DaemonInfo {
        #[arg(short, long)]
        json: bool,

        #[command(flatten)]
        oci_args: OciArgs,
    },
    #[command(alias = "du")]
    DiskUsage {
        #[arg(short, long)]
        json: bool,

        #[command(flatten)]
        oci_args: OciArgs,
    },
    Workers {
        #[arg(short, long)]
        json: bool,

        #[command(flatten)]
        oci_args: OciArgs,
    },
    /// Tmp for testing
    #[command(hide = true)]
    Solve,
}

impl DebugCommand {
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        match self {
            DebugCommand::DaemonInfo { json, oci_args } => {
                let mut client =
                    Client::connect(oci_args.oci_backend(), "cicada-buildkitd".into()).await?;
                let info = client.info().await?;

                if json {
                    let info = serde_json::json!({
                        "buildkit_version": info.buildkit_version.map(|v| {
                            serde_json::json!({
                                "package": v.package,
                                "version": v.version,
                                "revision": v.revision,
                            })
                        }),
                    });

                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else if let Some(buildkit_version) = info.buildkit_version {
                    println!(
                        "{} {} {} {}",
                        "Buildkit version:".bold(),
                        buildkit_version.version,
                        buildkit_version.package,
                        buildkit_version.revision
                    );
                } else {
                    anyhow::bail!("Buildkit version not found");
                }
            }
            DebugCommand::DiskUsage { json, oci_args } => {
                let mut client =
                    Client::connect(oci_args.oci_backend(), "cicada-buildkitd".into()).await?;
                let mut usage = client.disk_usage().await?;

                usage.record.sort_by_key(|r| -r.size);

                if json {
                    let json = serde_json::json!({
                        "record": usage
                            .record
                            .into_iter()
                            .map(|row| {
                                serde_json::json!({
                                "id": row.id,
                                "mutable": row.mutable,
                                "inUse": row.in_use,
                                "size": row.size,
                                "createdAt": row.created_at.as_ref().map(|t| t.to_string()),
                                "lastUsedAt": row.last_used_at.as_ref().map(|t| t.to_string()),
                                "usageCount": row.usage_count,
                                "description": row.description,
                                "recordType": row.record_type,
                                "shared": row.shared,
                                "parents": row.parents,
                                })
                            })
                            .collect::<Vec<_>>(),
                    });

                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!(
                        "{: <40} {: <12} {: <20} {: <10}",
                        "ID".bold(),
                        "RECLAIMABLE".bold(),
                        "SIZE".bold(),
                        "LAST ACCESSED".bold()
                    );

                    let mut total_size = 0;
                    let mut reclaimable_size = 0;

                    for row in usage.record {
                        total_size += row.size;
                        if !row.in_use {
                            reclaimable_size += row.size;
                        }

                        println!(
                            "{: <40} {: <12} {: <20} {: <10}",
                            row.id,
                            !row.in_use,
                            format_size(row.size as u64, DECIMAL),
                            row.last_used_at.map(|t| t.to_string()).unwrap_or_default()
                        );
                    }

                    println!();
                    println!(
                        "{}: {}",
                        "Reclaimable size".bold(),
                        format_size(reclaimable_size as u64, DECIMAL)
                    );
                    println!(
                        "{}: {}",
                        "Total size".bold(),
                        format_size(total_size as u64, DECIMAL)
                    );
                }
            }
            DebugCommand::Workers { oci_args, json } => {
                let mut client =
                    Client::connect(oci_args.oci_backend(), "cicada-buildkitd".into()).await?;
                let workers = client.list_workers().await?;

                if json {
                    let json = workers
                        .record
                        .into_iter()
                        .map(|row| {
                            serde_json::json!({
                                "id": row.id,
                                "labels": row.labels,
                                "platforms": row.platforms
                                    .iter()
                                    .map(|p| {
                                        serde_json::json!({
                                            "os": p.os,
                                            "architecture": p.architecture,
                                            "variant": if p.variant.is_empty() {
                                                None
                                            } else {
                                                Some(&p.variant)
                                            },
                                            "osVersion": if p.os_version.is_empty() {
                                                None
                                            } else {
                                                Some(&p.os_version)
                                            },
                                            "osFeatures": if p.os_features.is_empty() {
                                                None
                                            } else {
                                                Some(&p.os_features)
                                            },
                                        })
                                    })
                                    .collect::<Vec<_>>(),
                                "gcPolicy": row.gc_policy
                                    .iter()
                                    .map(|p| {
                                        serde_json::json!({
                                            "all": p.all,
                                            "keepDuration": p.keep_duration,
                                            "keepBytes": p.keep_bytes,
                                            "filters": p.filters,
                                        })
                                    })
                                    .collect::<Vec<_>>(),
                                "buildkitVersion": row.buildkit_version
                                    .map(|v| {
                                        serde_json::json!({
                                            "package": v.package,
                                            "version": v.version,
                                            "revision": v.revision,
                                        })
                                    }),
                            })
                        })
                        .collect::<Vec<_>>();

                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!("{: <40} {: <40}", "ID".bold(), "PLATFORMS".bold(),);

                    for worker in workers.record {
                        println!(
                            "{: <40} {: <40}",
                            worker.id,
                            worker
                                .platforms
                                .iter()
                                .map(|p| format!("{}/{}", p.os, p.architecture))
                                .collect::<Vec<_>>()
                                .join(",")
                        );
                    }
                }
            }

            DebugCommand::Solve => {
                use buildkit_rs::llb::*;

                let builder_image =
                    Image::new("alpine:latest").with_custom_name("image - alpine:latest");

                let local = Local::new("local".into())
                    .with_custom_name("local source")
                    .with_exclude("target");

                let command = Exec::shell(
                    "/bin/sh",
                    "echo 'this is custom logging!!!' && sleep 1 && ls -al /src && cat /run/secrets/abc",
                )
                .with_custom_name(
                    "shell - echo 'this is custom logging!!!' && sleep 1 && echo 'hey'",
                )
                .with_mount(Mount::layer_readonly(builder_image.output(), "/"))
                .with_mount(Mount::layer_readonly(local.output(), "/src"))
                .with_mount(Mount::scratch("/out", 0))
                .with_mount(Mount::secret("/run/secrets/abc", "abc", 0, 0, 0o600, false))
                .ignore_cache(true);

                let definition: Definition = Definition::new(command.output(0));

                let mut client =
                    Client::connect(OciBackend::Docker, "cicada-buildkitd".into()).await?;

                let session = client
                    .session(SessionOptions {
                        name: "cicada".into(),
                        local: [("local".into(), ".".into())].into_iter().collect(),
                        secrets: vec![("abc".into(), SecretSource::Memory("abc".into()))]
                            .into_iter()
                            .collect(),
                    })
                    .await
                    .unwrap();

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                let id = random_id();

                let mut res = client.status(id.clone()).await.unwrap();

                tokio::spawn(async move {
                    while let Some(event) = res.next().await {
                        // dbg!(&event);
                        match event {
                            Ok(StatusResponse {
                                vertexes,
                                statuses: _,
                                logs,
                                warnings,
                            }) => {
                                for Vertex {
                                    digest,
                                    // inputs,
                                    name,
                                    cached,
                                    // started,
                                    completed,
                                    // error,
                                    // progress_group,
                                    ..
                                } in vertexes
                                {
                                    // let msg_str = String::from_utf8_lossy(&name);
                                    if completed.is_some() {
                                        info!(%cached, "{digest}: {name}");
                                    }
                                }

                                for VertexLog {
                                    vertex,
                                    // timestamp,
                                    // stream,
                                    msg,
                                    ..
                                } in logs
                                {
                                    let msg_str = String::from_utf8_lossy(&msg);
                                    for line in msg_str.lines() {
                                        info!("{vertex}: log: {line}");
                                    }
                                }

                                for VertexWarning {
                                    vertex,
                                    // level,
                                    short,
                                    // detail,
                                    // url,
                                    // info,
                                    // ranges,
                                    ..
                                } in warnings
                                {
                                    let short = String::from_utf8_lossy(&short);
                                    warn!("{vertex}: {short}");
                                }
                            }
                            Err(e) => {
                                error!("{:#?}", e);
                            }
                        }
                    }
                });

                let res = client
                    .solve(SolveOptions {
                        id: id.clone(),
                        session: session.id.clone(),
                        definition,
                    })
                    .await;

                info!(?res);
            }
        }

        Ok(())
    }
}
