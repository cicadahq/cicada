use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use once_cell::sync::Lazy;

/// Data is something persisted between installs
pub fn data_path() -> Result<PathBuf> {
    let path = dirs::data_local_dir()
        .context("Could not find data local dir")?
        .join("cicada");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(path)
}

static TELEMETRY_ENABLED: Lazy<bool> = Lazy::new(|| {
    let musl = cfg!(target_env = "musl");
    let debug = cfg!(debug_assertions);
    let disabled = std::env::var_os("CICADA_DISABLE_TELEMETRY").is_some();

    !musl && !debug && !disabled
});

pub fn telemetry_enabled() -> bool {
    *TELEMETRY_ENABLED
}
