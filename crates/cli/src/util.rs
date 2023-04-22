use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use base64::prelude::*;
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

#[allow(dead_code)]
/// A base64 encoded sha256 digest
pub fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let bytes = hasher.finalize().to_vec();
    BASE64_STANDARD.encode(bytes)
}
