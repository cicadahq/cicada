use std::fmt;

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum OciBackend {
    Docker,
    Podman,
}

impl OciBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            OciBackend::Docker => "docker",
            OciBackend::Podman => "podman",
        }
    }
}

impl fmt::Display for OciBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
