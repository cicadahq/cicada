use std::fmt;

use buildkit_rs::util::oci::OciBackend;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Default)]
pub enum OciBackendClap {
    #[default]
    Docker,
    Podman,
}

impl fmt::Display for OciBackendClap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OciBackendClap::Docker => f.write_str("docker"),
            OciBackendClap::Podman => f.write_str("podman"),
        }
    }
}

impl From<OciBackendClap> for OciBackend {
    fn from(backend: OciBackendClap) -> Self {
        match backend {
            OciBackendClap::Docker => OciBackend::Docker,
            OciBackendClap::Podman => OciBackend::Podman,
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct OciArgs {
    /// The OCI backend to use
    #[arg(long, default_value_t = OciBackendClap::default(), env = "CICADA_OCI_BACKEND")]
    pub oci_backend: OciBackendClap,
}

impl OciArgs {
    pub fn oci_backend(self) -> OciBackend {
        self.oci_backend.into()
    }
}
