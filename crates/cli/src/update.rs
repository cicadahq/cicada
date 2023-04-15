use self_update::update::ReleaseUpdate;
use tracing::info;

#[cfg(target_env = "musl")]
compile_error!("Musl does not support self-update");

/// Check for a new version of Cicada and print a message if there is one
pub async fn check_for_update() {
    use std::time::SystemTime;

    use owo_colors::OwoColorize;

    use crate::util::data_path;

    let print_update_msg = |version: &str| {
        let bold_yellow = owo_colors::Style::new().bold().yellow();
        info!(
            "\n{}{}\n{}{}\n",
            "A new version of Cicada is available: "
                .if_supports_color(atty::Stream::Stdout, |s| s.yellow()),
            version.if_supports_color(atty::Stream::Stdout, |s| s.style(bold_yellow)),
            "Run to update: ".if_supports_color(atty::Stream::Stdout, |s| s.yellow()),
            "cicada update".if_supports_color(atty::Stream::Stdout, |s| s.style(bold_yellow))
        );
    };

    let Ok(data_path) = data_path() else {
        return;
    };

    let last_update_check_path = data_path.join("last-update-check");
    let latest_release_path = data_path.join("latest-release");

    // Check the last time we checked for an update
    if let Ok(last_update_check) = std::fs::read_to_string(&last_update_check_path) {
        let last_update_check: SystemTime = std::time::UNIX_EPOCH
            + std::time::Duration::from_secs(last_update_check.parse().unwrap());

        if last_update_check.elapsed().unwrap_or_default().as_secs() < 60 * 60 * 24 {
            // Check the latest release file to see if we have the latest version
            if let Ok(latest_release) = std::fs::read_to_string(&latest_release_path) {
                let latest_release: semver::Version = latest_release
                    .parse()
                    .unwrap_or_else(|_| semver::Version::new(0, 0, 0));

                if latest_release > semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap() {
                    print_update_msg(&latest_release.to_string());
                }
            }

            return;
        }
    }

    // Write the current time to the last update check file
    std::fs::write(
        &last_update_check_path,
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string(),
    )
    .unwrap();

    let Ok(Ok(latest_release)) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<self_update::update::Release> {
            let status = self_update_release()?.get_latest_release()?;
            Ok(status)
        })
        .await else {
        return;
    };

    // Write the latest release version to the latest release file
    std::fs::write(&latest_release_path, &latest_release.version).ok();

    let Ok(latest_semver) = semver::Version::parse(&latest_release.version) else {
        return;
    };

    let Ok(current_semver) = semver::Version::parse(env!("CARGO_PKG_VERSION")) else {
        return;
    };

    if latest_semver > current_semver {
        print_update_msg(&latest_release.version);
    }
}

pub fn self_update_release() -> anyhow::Result<Box<dyn ReleaseUpdate>> {
    let bin_name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "cicada-x86_64-unknown-linux-gnu.tar.gz",
        ("macos", "x86_64") => "cicada-x86_64-apple-darwin.tar.gz",
        ("macos", "aarch64") => "cicada-aarch64-apple-darwin.tar.gz",
        ("windows", "x86_64") => "cicada-x86_64-pc-windows-msvc.zip",
        _ => anyhow::bail!("Unsupported OS"),
    };

    let release_update = self_update::backends::github::Update::configure()
        .repo_owner("cicadahq")
        .repo_name("cicada")
        .bin_name(bin_name)
        .bin_path_in_archive("cicada")
        .show_download_progress(true)
        .current_version(self_update::cargo_crate_version!())
        .build()?;

    Ok(release_update)
}
