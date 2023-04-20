use std::path::PathBuf;

use semver::{Version, VersionReq};
use tokio::process::Command;

pub const DENO_VERSION: &str = "1.32.5";
pub const DENO_VERSION_REQ: &str = ">=1.32";

#[cfg(feature = "managed-bins")]
pub const BUILDCTL_VERSION: &str = "0.11.5";
pub const BUILDCTL_VERSION_REQ: &str = ">=0.11";

pub fn deno_version_req() -> VersionReq {
    VersionReq::parse(DENO_VERSION_REQ).expect("Invalid DENO_VERSION_REQ")
}

async fn path_deno_version() -> Option<Version> {
    let deno_version = Command::new("deno").arg("-V").output().await.ok()?.stdout;
    let deno_version = String::from_utf8(deno_version).ok()?;
    let deno_version = deno_version.trim();
    let deno_trimmed = deno_version.strip_prefix("deno ").unwrap_or(deno_version);
    let demo_semver = Version::parse(deno_trimmed).ok()?;

    Some(demo_semver)
}

pub fn buildctl_version_req() -> VersionReq {
    VersionReq::parse(BUILDCTL_VERSION_REQ).expect("Invalid BUILDCTL_VERSION_REQ")
}

async fn path_buildctl_version() -> Option<Version> {
    let buildctl_version = Command::new("buildctl")
        .arg("-v")
        .output()
        .await
        .ok()?
        .stdout;
    let buildctl_version = String::from_utf8(buildctl_version).ok()?;
    let buildctl_version = buildctl_version.trim();
    let buildctl_trimmed = buildctl_version.split_whitespace().nth(2)?;
    let buildctl_semver = Version::parse(buildctl_trimmed).ok()?;

    Some(buildctl_semver)
}

#[cfg(feature = "managed-bins")]
fn managed_deno_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::util::data_path()?.join("deno"))
}

#[cfg(feature = "managed-bins")]
fn managed_deno_exe() -> anyhow::Result<PathBuf> {
    Ok(managed_deno_dir()?.join(format!("deno-{DENO_VERSION}")))
}

#[cfg(feature = "managed-bins")]
fn managed_buildctl_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::util::data_path()?.join("buildctl"))
}

#[cfg(feature = "managed-bins")]
fn managed_buildctl_exe() -> anyhow::Result<PathBuf> {
    Ok(managed_buildctl_dir()?.join(format!("buildctl-{BUILDCTL_VERSION}")))
}

#[cfg(feature = "managed-bins")]
fn deno_download_link() -> anyhow::Result<String> {
    let deno_archive_name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "deno-x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "deno-x86_64-apple-darwin",
        ("macos", "aarch64") => "deno-aarch64-apple-darwin",
        ("windows", "x86_64") => "deno-x86_64-pc-windows-msvc",
        _ => anyhow::bail!("Unsupported platform"),
    };

    Ok(format!(
        "https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/{deno_archive_name}.zip"
    ))
}

#[cfg(feature = "managed-bins")]
pub async fn download_deno_exe() -> anyhow::Result<PathBuf> {
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    // otherwise download the managed version if it doesn't exist
    let managed_deno_exe = managed_deno_exe()?;
    if managed_deno_exe.exists() {
        return Ok(managed_deno_exe);
    }

    let managed_deno_dir = managed_deno_dir()?;

    // clear the directory if it exists
    if managed_deno_dir.exists() {
        tokio::fs::remove_dir_all(&managed_deno_dir).await?;
    }
    std::fs::create_dir_all(&managed_deno_dir)?;

    let deno_download_link = deno_download_link()?;

    let mut tempfile = tokio::fs::File::from_std(tempfile::tempfile()?);

    let mut deno_archive_res = reqwest::get(&deno_download_link).await?;

    let download_size = deno_archive_res.content_length().unwrap_or_default();

    let spinner = indicatif::ProgressBar::new(download_size);
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(&format!("{{spinner:.blue}}  Downloading deno v{DENO_VERSION} [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} (eta {{eta}})"))
        ?
    );

    while let Some(chunk) = deno_archive_res.chunk().await? {
        tempfile.write_all(&chunk).await?;

        spinner.inc(chunk.len() as u64);
    }
    tempfile.flush().await?;

    spinner.finish_and_clear();
    eprintln!("✅ Downloaded deno v{DENO_VERSION}");

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner().template(&format!(
            "{{spinner:.blue}}  Extracting deno v{DENO_VERSION}"
        ))?,
    );

    let file = tempfile.into_std().await;

    let mut deno_archive = zip::ZipArchive::new(file)?;

    let mut deno_exe_zip = deno_archive.by_name("deno")?;

    let mut deno_exe_file = std::fs::File::create(&managed_deno_exe)?;

    std::io::copy(&mut deno_exe_zip, &mut deno_exe_file)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&managed_deno_exe)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&managed_deno_exe, perms)?;
    }

    spinner.finish_and_clear();
    eprintln!("✅ Installed deno v{DENO_VERSION}");

    Ok(managed_deno_exe)
}

#[cfg(feature = "managed-bins")]
fn buildctl_download_link() -> anyhow::Result<String> {
    let deno_archive_name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => format!("buildkit-v{BUILDCTL_VERSION}.linux-amd64"),
        ("macos", "x86_64") => format!("buildkit-v{BUILDCTL_VERSION}.darwin-amd64"),
        ("macos", "aarch64") => format!("buildkit-v{BUILDCTL_VERSION}.darwin-arm64"),
        ("windows", "x86_64") => format!("buildkit-v{BUILDCTL_VERSION}.windows-amd64"),
        _ => anyhow::bail!("Unsupported platform"),
    };

    Ok(format!(
        "https://github.com/moby/buildkit/releases/download/v{BUILDCTL_VERSION}/{deno_archive_name}.tar.gz"
    ))
}

#[cfg(feature = "managed-bins")]
pub async fn download_buildctl_exe() -> anyhow::Result<PathBuf> {
    use std::{io::Write, time::Duration};

    // otherwise download the managed version if it doesn't exist
    let managed_buildctl_exe = managed_buildctl_exe()?;
    if managed_buildctl_exe.exists() {
        return Ok(managed_buildctl_exe);
    }

    let managed_buildctl_dir = managed_buildctl_dir()?;

    // clear the directory if it exists
    if managed_buildctl_dir.exists() {
        tokio::fs::remove_dir_all(&managed_buildctl_dir).await?;
    }
    std::fs::create_dir_all(&managed_buildctl_dir)?;

    let buildctl_download_link = buildctl_download_link()?;

    let mut tempfile = tempfile::NamedTempFile::new()?;

    let mut buildctl_archive_res = reqwest::get(&buildctl_download_link).await?;

    buildctl_archive_res.error_for_status_ref()?;

    let download_size = buildctl_archive_res.content_length().unwrap_or_default();

    let spinner = indicatif::ProgressBar::new(download_size);
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(&format!("{{spinner:.blue}}  Downloading buildctl v{BUILDCTL_VERSION} [{{wide_bar:.cyan/blue}}] {{bytes}}/{{total_bytes}} (eta {{eta}})"))
        ?
    );

    while let Some(chunk) = buildctl_archive_res.chunk().await? {
        tempfile.write_all(&chunk)?;

        spinner.inc(chunk.len() as u64);
    }
    tempfile.flush()?;

    spinner.finish_and_clear();
    eprintln!("✅ Downloaded buildctl v{BUILDCTL_VERSION}");

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner().template(&format!(
            "{{spinner:.blue}}  Extracting buildctl v{BUILDCTL_VERSION}"
        ))?,
    );

    // let compressed_archive = tempfile.into_std().await;
    // let archive_decoder =
    //     flate2::bufread::GzDecoder::new(std::io::BufReader::new(&compressed_archive));

    // let unpacked_dir = tempfile::tempdir()?;

    // tar::Archive::new(archive_decoder)
    //     .unpack(unpacked_dir.path())
    //     .context("Failed to unpack buildctl archive")?;

    // libs arent working, we are going to use `Command` instead for now

    let tempdir = tempfile::tempdir()?;

    Command::new("tar")
        .arg("xzf")
        .arg(tempfile.path())
        .arg("-C")
        .arg(tempdir.path())
        .output()
        .await?;

    let buildctl_path = tempdir.path().join("bin").join("buildctl");

    // Print the contents of the archive
    Command::new("ls")
        .arg("-l")
        .arg(&buildctl_path)
        .spawn()?
        .wait()
        .await?;

    // #[cfg(unix)]
    // {
    //     use std::os::unix::fs::PermissionsExt;
    //     let mut perms = std::fs::metadata(&managed_buildctl_exe)?.permissions();
    //     perms.set_mode(0o755);
    //     std::fs::set_permissions(&managed_buildctl_exe, perms)?;
    // }

    tokio::fs::copy(buildctl_path, &managed_buildctl_exe).await?;

    drop(tempdir);

    spinner.finish_and_clear();
    eprintln!("✅ Installed buildctl v{BUILDCTL_VERSION}");

    Ok(managed_buildctl_exe)
}

pub async fn deno_exe() -> anyhow::Result<PathBuf> {
    // Check if the deno version is already satisfied by the one in the path
    if let Some(deno_version) = path_deno_version().await {
        if deno_version_req().matches(&deno_version) {
            return Ok(PathBuf::from("deno"));
        }
    }

    // otherwise download the managed version if it doesn't exist
    #[cfg(feature = "managed-bins")]
    let exe = download_deno_exe().await?;
    #[cfg(feature = "managed-bins")]
    return Ok(exe);

    #[cfg(not(feature = "managed-bins"))]
    return Err(anyhow::anyhow!("Cicada requires Deno {DENO_VERSION_REQ} to run. Please install it using one of the methods on https://deno.land/manual/getting_started/installation"));
}

pub async fn buildctl_exe() -> anyhow::Result<PathBuf> {
    // Check if the buildctl version is already satisfied by the one in the path
    if let Some(buildctl_version) = path_buildctl_version().await {
        if buildctl_version_req().matches(&buildctl_version) {
            return Ok(PathBuf::from("buildctl"));
        }
    }

    // otherwise download the managed version if it doesn't exist
    #[cfg(feature = "managed-bins")]
    let exe = download_buildctl_exe().await?;
    #[cfg(feature = "managed-bins")]
    return Ok(exe);

    #[cfg(not(feature = "managed-bins"))]
    return Err(anyhow::anyhow!(
        "Cicada requires buildctl {BUILDCTL_VERSION_REQ} to run."
    ));
}

#[cfg(test)]
#[cfg(feature = "managed-bins")]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deno_version() {
        deno_version_req();
        dbg!(path_deno_version().await.unwrap());
    }

    #[tokio::test]
    async fn buildctl_version() {
        buildctl_version_req();
        dbg!(path_buildctl_version().await.unwrap());
    }

    #[tokio::test]
    async fn test_download_deno() {
        // Remove the managed deno dir if it exists
        let managed_deno_dir = managed_deno_dir().unwrap();
        if managed_deno_dir.exists() {
            tokio::fs::remove_dir_all(&managed_deno_dir).await.unwrap();
        }

        let _download_res = download_deno_exe().await.unwrap();

        let deno_exe_path = managed_deno_exe().unwrap();
        assert!(deno_exe_path.is_file());

        // Run deno -V to check the version
        let deno_version_stdout = Command::new(&deno_exe_path)
            .arg("-V")
            .output()
            .await
            .unwrap()
            .stdout;

        let deno_version_str = String::from_utf8(deno_version_stdout).unwrap();
        let deno_version_str = deno_version_str.trim();
        let deno_trimmed = deno_version_str
            .strip_prefix("deno ")
            .unwrap_or(deno_version_str);

        assert_eq!(DENO_VERSION, deno_trimmed);
    }

    #[tokio::test]
    async fn test_download_buildctl() {
        // Remove the managed buildctl dir if it exists
        let managed_buildctl_dir = managed_buildctl_dir().unwrap();
        if managed_buildctl_dir.exists() {
            tokio::fs::remove_dir_all(&managed_buildctl_dir)
                .await
                .unwrap();
        }

        let _download_res = download_buildctl_exe().await.unwrap();

        let buildctl_exe_path = managed_buildctl_exe().unwrap();
        assert!(buildctl_exe_path.is_file());

        // Run buildctl -v to check the version
        let buildctl_version_stdout = Command::new(&buildctl_exe_path)
            .arg("-v")
            .output()
            .await
            .unwrap()
            .stdout;

        let buildctl_version_str = String::from_utf8(buildctl_version_stdout).unwrap();
        let buildctl_version_str = buildctl_version_str.trim();
        let buildctl_version_str = buildctl_version_str.split_whitespace().nth(2).unwrap();
        let buildctl_version_str = buildctl_version_str
            .strip_prefix("v")
            .unwrap_or(buildctl_version_str);

        assert_eq!(BUILDCTL_VERSION, buildctl_version_str);
    }
}
