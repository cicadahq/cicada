use std::path::PathBuf;

use tokio::process::Command;

use crate::{print_error, util::data_path, DENO_VERSION};

pub async fn download_deno() -> anyhow::Result<PathBuf> {
    let deno_version_dir = data_path()?.join("deno-bin").join(DENO_VERSION);
    if !deno_version_dir.exists() {
        std::fs::create_dir_all(&deno_version_dir)?;
    }

    let deno_exe_path = deno_version_dir.join("deno-x86_64-unknown-linux-gnu");

    if !deno_exe_path.exists() {
        println!("Downloading deno for release v{DENO_VERSION}");

        // Clean up any old versions
        for file in std::fs::read_dir(&data_path()?.join("deno-bin"))? {
            let file = file?;
            let file_name = file.file_name();
            let file_name = file_name.to_str().unwrap();
            if file_name != DENO_VERSION {
                std::fs::remove_dir_all(file.path())?;
            }
        }

        let deno_tar = "deno-x86_64-unknown-linux-gnu.zip".to_string();
        let deno_tar_path = deno_version_dir.join(&deno_tar);

        if !deno_tar_path.exists() {
            // TODO: Replace with reqwest
            let deno_status = Command::new("curl")
                .args([
                    "-fSsL",
                    format!("https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/deno-x86_64-unknown-linux-gnu.zip")
                    .as_str(),
                    "-o",
                    deno_tar_path.to_str().unwrap(),
                ])
                .spawn()
                .unwrap()
                .wait()
                .await?;

            if !deno_status.success() {
                print_error("Failed to download cicada release");
                std::process::exit(1);
            }
        }

        // TODO: Replace with zip crate
        let unzip_status = Command::new("unzip")
            .args([
                "-q",
                "-o",
                deno_tar_path.to_str().unwrap(),
                "-d",
                deno_version_dir.to_str().unwrap(),
            ])
            .spawn()
            .unwrap()
            .wait()
            .await?;

        if !unzip_status.success() {
            print_error("Failed to unpack deno release");
            std::process::exit(1);
        }

        // Move the cicada binary to the bin directory
        std::fs::rename(
            deno_version_dir.join("deno"),
            deno_version_dir.join("deno-x86_64-unknown-linux-gnu"),
        )?;

        // Delete the tarball
        std::fs::remove_file(deno_tar_path)?;
    }

    Ok(deno_exe_path)
}

pub async fn download_cicada_musl() -> anyhow::Result<PathBuf> {
    let version = env!("CARGO_PKG_VERSION");

    let version_bin_dir = data_path()?.join("cicada-bin").join(version);
    if !version_bin_dir.exists() {
        std::fs::create_dir_all(&version_bin_dir)?;
    }

    let linux_exe_name = "cicada-x86_64-unknown-linux-musl";
    let linux_exe_path = version_bin_dir.join(linux_exe_name);
    let linux_tar = format!("{linux_exe_name}.tar.gz");
    let linux_tar_path = version_bin_dir.join(&linux_tar);

    if !linux_exe_path.exists() {
        println!("Downloading cicada runner for release v{version}");

        // Clean up any old versions
        for file in std::fs::read_dir(&data_path()?.join("cicada-bin"))? {
            let file = file?;
            let file_name = file.file_name();
            let file_name = file_name.to_str().unwrap();
            if file_name != version {
                std::fs::remove_dir_all(file.path())?;
            }
        }

        if !linux_tar_path.exists() {
            // TODO: Replace with reqwest
            let curl_status = Command::new("curl")
                .args([
                    "-fSsL",
                    format!(
                        "https://github.com/cicadahq/cicada/releases/download/v{version}/{linux_tar}"
                    )
                    .as_str(),
                    "-o",
                    linux_tar_path.to_str().unwrap(),
                ])
                .spawn()
                .unwrap()
                .wait()
                .await?;

            if !curl_status.success() {
                print_error("Failed to download cicada release");
                std::process::exit(1);
            }
        }

        // TODO: Replace with tar/flate2 crate
        let tar_status = Command::new("tar")
            .args([
                "xzf",
                version_bin_dir.join(linux_tar).to_str().unwrap(),
                "-C",
                version_bin_dir.to_str().unwrap(),
            ])
            .spawn()
            .unwrap()
            .wait()
            .await?;

        if !tar_status.success() {
            print_error("Failed to unpack cicada release");
            std::process::exit(1);
        }

        // Move the cicada binary to the bin directory
        std::fs::rename(
            version_bin_dir.join("cicada"),
            version_bin_dir.join(linux_exe_name),
        )?;

        // Delete the tarball
        std::fs::remove_file(linux_tar_path)?;
    }

    Ok(linux_exe_path)
}
