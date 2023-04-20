use std::fmt::Debug;

use buildkit_rs::{
    client::{Client, SolveOptions},
    llb::{
        Definition, Exec, Image, Mount, MultiBorrowedOutput, OpMetadataBuilder,
        SingleBorrowedOutput,
    },
    util::oci::OciBackend,
};
use owo_colors::OwoColorize;

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
    ListWorkers {
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
                    let buildkit_version = info.buildkit_version.map(|v| {
                        serde_json::json!({
                            "package": v.package,
                            "version": v.version,
                            "revision": v.revision,
                        })
                    });

                    let info = serde_json::json!({
                        "buildkit_version": buildkit_version,
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
                let usage = client.disk_usage().await?;

                if json {
                    let json = serde_json::json!({
                        "record": usage
                            .record
                            .into_iter()
                            .map(|row| {
                                serde_json::json!({
                                "id": row.id,
                                "mutable": row.mutable,
                                "in_use": row.in_use,
                                "size": row.size,
                                // "created_at": row.created_at,
                                // "last_used_at": row.last_used_at,
                                "usage_count": row.usage_count,
                                "description": row.description,
                                "record_type": row.record_type,
                                "shared": row.shared,
                                "parents": row.parents,
                                })
                            })
                            .collect::<Vec<_>>(),
                    });

                    println!("{}", serde_json::to_string_pretty(&json)?);
                } else {
                    println!(
                        "{: <40} {: <10} {: <10} {: <10}",
                        "ID".bold(),
                        "RECLAIMABLE".bold(),
                        "SIZE".bold(),
                        "LAST ACCESSED".bold()
                    );

                    let mut total_size = 0;
                    let mut reclaimable_size = 0;

                    for row in usage.record {
                        total_size += row.size;
                        if row.in_use {
                            reclaimable_size += row.size;
                        }

                        println!(
                            "{: <40} {: <10} {: <10} {: <10}",
                            row.id,
                            row.in_use,
                            row.size,
                            row.last_used_at.map(|t| t.to_string()).unwrap_or_default()
                        );
                    }

                    println!("{}: {reclaimable_size}", "Reclaimable size".bold());
                    println!("{}: {total_size}", "Total size".bold());
                }
            }
            DebugCommand::ListWorkers { oci_args, .. } => {
                let mut client =
                    Client::connect(oci_args.oci_backend(), "cicada-buildkitd".into()).await?;
                let workers = client.list_workers().await?;
                dbg!(workers);
            }

            DebugCommand::Solve => {
                let builder_image = Image::new("alpine:latest")
                    .with_custom_name("Using alpine:latest as a builder");

                let command = Exec::shlex("/bin/sh -c \"echo 'hello world'\"")
                    .with_custom_name("create a dummy file")
                    .with_mount(Mount::layer_readonly(builder_image.output(), "/"))
                    .with_mount(Mount::scratch("/out", 0));

                let definition: Definition = Definition::new(command.output(0));

                let mut client =
                    Client::connect(OciBackend::Docker, "cicada-buildkitd".into()).await?;
                let res = client
                    .solve(SolveOptions {
                        id: "123".into(),
                        definition,
                    })
                    .await;

                dbg!(res).unwrap();
            }
        }

        Ok(())
    }
}
