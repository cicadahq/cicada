use std::sync::Arc;

use sentry::ClientInitGuard;

use super::{sentry_enabled, SENTRY_AUTH_TOKEN};

pub(crate) fn sentry_init() -> Option<ClientInitGuard> {
    sentry_enabled()
        .then_some(SENTRY_AUTH_TOKEN)
        .flatten()
        .map(|token| {
            sentry::init((
                token,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    before_send: Some(Arc::new(|mut event| {
                        // This is too identifying
                        event.server_name = None;
                        Some(event)
                    })),
                    ..Default::default()
                },
            ))
        })
}
