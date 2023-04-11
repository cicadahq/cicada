use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

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
