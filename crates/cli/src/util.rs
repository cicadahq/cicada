use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use sha2::{Digest, Sha256};

/// Data is something persisted between installs
#[allow(dead_code)]
pub fn data_path() -> Result<PathBuf> {
    let path = dirs::data_local_dir()
        .context("Could not find data local dir")?
        .join("cicada");
    if !path.exists() {
        std::fs::create_dir_all(&path)?;
    }

    Ok(path)
}

pub fn digest(bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().to_vec()
}
