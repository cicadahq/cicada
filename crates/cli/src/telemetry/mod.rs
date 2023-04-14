pub(crate) mod segment;
pub(crate) mod sentry;

use once_cell::sync::Lazy;

const SEGMENT_WRITE_KEY: Option<&str> = option_env!("SEGMENT_WRITE_KEY");

static SEGMENT_ENABLED: Lazy<bool> = Lazy::new(|| {
    let musl = cfg!(target_env = "musl");
    let debug = cfg!(debug_assertions);
    let disabled = std::env::var_os("CICADA_DISABLE_TELEMETRY").is_some();
    let has_write_key = SEGMENT_WRITE_KEY.is_some();

    !musl && !debug && !disabled && has_write_key
});

/// Segment tracks event based analytics
pub fn segment_enabled() -> bool {
    *SEGMENT_ENABLED
}

const SENTRY_AUTH_TOKEN: Option<&str> = option_env!("SENTRY_AUTH_TOKEN");

static SENTRY_ENABLED: Lazy<bool> = Lazy::new(|| {
    let debug = cfg!(debug_assertions);
    let disabled = std::env::var_os("CICADA_DISABLE_SENTRY").is_some();
    let has_auth_token = SENTRY_AUTH_TOKEN.is_some();

    !debug && !disabled && has_auth_token
});

/// Sentry tracks error based analytics
pub fn sentry_enabled() -> bool {
    *SENTRY_ENABLED
}
