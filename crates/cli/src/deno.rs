use std::path::PathBuf;

use semver::{Version, VersionReq};
use tokio::process::Command;

pub const DENO_VERSION: &str = "1.32.4";
pub const DENO_VERSION_REQ: &str = ">=1.32";

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

#[cfg(feature = "managed-deno")]
fn managed_deno_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::util::data_path()?.join("deno"))
}

#[cfg(feature = "managed-deno")]
fn managed_deno_exe() -> anyhow::Result<PathBuf> {
    Ok(managed_deno_dir()?.join(format!("deno-{DENO_VERSION}")))
}

#[cfg(feature = "managed-deno")]
fn deno_download_link() -> String {
    let deno_archive_name = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "deno-x86_64-unknown-linux-gnu",
        ("macos", "x86_64") => "deno-x86_64-apple-darwin",
        ("macos", "aarch64") => "deno-aarch64-apple-darwin",
        ("windows", "x86_64") => "deno-x86_64-pc-windows-msvc.zip",
        _ => panic!("Unsupported platform"),
    };

    format!(
        "https://github.com/denoland/deno/releases/download/v{DENO_VERSION}/{deno_archive_name}.zip"
    )
}

#[cfg(feature = "managed-deno")]
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

    let deno_download_link = deno_download_link();

    let mut tempfile = tokio::fs::File::from_std(tempfile::tempfile()?);

    let mut deno_archive_res = reqwest::get(&deno_download_link).await?;

    let download_size = deno_archive_res
        .content_length()
        .expect("content-length header is required");

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
    eprintln!("✅ Extracted deno v{DENO_VERSION}");

    Ok(managed_deno_exe)
}

pub async fn deno_exe() -> anyhow::Result<PathBuf> {
    // Check if the deno version is already satisfied by the one in the path
    if let Some(deno_version) = path_deno_version().await {
        if deno_version_req().matches(&deno_version) {
            return Ok(PathBuf::from("deno"));
        }
    }

    // otherwise download the managed version if it doesn't exist
    #[cfg(feature = "managed-deno")]
    let exe = download_deno_exe().await?;
    #[cfg(feature = "managed-deno")]
    return Ok(exe);

    #[cfg(not(feature = "managed-deno"))]
    return Err(anyhow::anyhow!("Cicada requires Deno {DENO_VERSION_REQ} to run. Please install it using one of the methods on https://deno.land/manual/getting_started/installation"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "managed-deno")]
    async fn test_download_deno() {
        // Remove the managed deno dir if it exists
        let managed_deno_dir = managed_deno_dir().unwrap();
        if managed_deno_dir.exists() {
            tokio::fs::remove_dir_all(&managed_deno_dir).await.unwrap();
        }

        let download_res = download_deno_exe().await;
        assert!(download_res.is_ok());

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

    #[test]
    #[cfg(feature = "managed-deno")]
    fn deno_version_assert() {
        deno_version_req();
    }
}
