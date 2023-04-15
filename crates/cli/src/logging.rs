use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write,
    hash::{Hash, Hasher},
};

use ahash::HashMap;
use anyhow::Result;
use tracing::{field::Visit, Event, Level, Subscriber};
use tracing_core::Field;
use tracing_subscriber::{
    fmt::SubscriberBuilder, layer::Context, prelude::__tracing_subscriber_SubscriberExt,
    registry::LookupSpan, util::SubscriberInitExt, Layer,
};

#[derive(Debug, Default)]
struct EventVisitor {
    output: String,
}

impl Visit for EventVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() == "message" {
            true => writeln!(&mut self.output, "{:?}", value).ok(),
            false => writeln!(&mut self.output, "  {}={:?}", field.name(), value).ok(),
        };
    }
}

#[derive(Debug, Default)]
struct SpanVisitor(pub HashMap<String, String>);

impl Visit for SpanVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, _: &Field, _: &dyn std::fmt::Debug) {}
}

pub struct CustomFormatLayer {}

impl CustomFormatLayer {
    pub fn new() -> Self {
        Self {}
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for CustomFormatLayer {
    fn on_new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing_core::span::Id,
        ctx: Context<'_, S>,
    ) {
        let span = ctx.span(id).unwrap();
        let mut visitor = SpanVisitor::default();
        attrs.record(&mut visitor);
        span.extensions_mut().insert(visitor);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let log_level = *metadata.level();

        if log_level > Level::INFO {
            return;
        }

        if let Some(current_span) = ctx.current_span().id() {
            let span = ctx.span(current_span).unwrap();
            if let Some(visitor) = span.extensions().get::<SpanVisitor>() {
                let job_name = visitor
                    .0
                    .get("job_name")
                    .map(|n| n.as_str())
                    .unwrap_or("unnamed_job");
                let mut hasher = DefaultHasher::new();
                job_name.hash(&mut hasher);
                let hash = hasher.finish();
                let color = (hash % 5) + 32 + (hash % 2) * 60;
                print!("\x1b[{color}m{job_name}\x1b[0m: ");
            };
        }

        match log_level {
            Level::ERROR => print!("\x1b[31m[error]\x1b[0m "),
            Level::WARN => print!("\x1b[33m[warn]\x1b[0m "),
            _ => (),
        }

        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        print!("{}", visitor.output);
    }
}

pub fn logging_init() -> Result<()> {
    let log_json = std::env::var_os("CICADA_LOG_JSON").is_some();
    match log_json {
        true => {
            tracing::subscriber::set_global_default(SubscriberBuilder::default().json().finish())?
        }
        false => tracing_subscriber::registry()
            .with(CustomFormatLayer::new())
            .try_init()?,
    }

    Ok(())
}
