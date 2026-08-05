//! Tracing layer that forwards logs to tklog.
//!
//! This module provides a tracing layer that forwards all log events to tklog,
//! enabling tklog's features like file rotation, compression, and structured logging.

use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A tracing layer that forwards log events to tklog.
pub struct TklogLayer;

impl TklogLayer {
    pub fn new() -> Self {
        // Configure tklog to write to file
        // Use set_cutmode_by_time for daily rotation
        tklog::LOG
            .set_console(true)
            .set_level(tklog::LEVEL::Debug)
            .set_cutmode_by_time("logs/memos-tklog.log", tklog::MODE::DAY, 0, true);

        Self
    }
}

impl<S: Subscriber> Layer<S> for TklogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level();

        // Extract fields from the event
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let message = visitor.message.unwrap_or_default();
        let target = metadata.target();
        let file = metadata.file().unwrap_or("unknown");
        let line = metadata.line().unwrap_or(0);

        // Format the log message
        let formatted = if message.is_empty() {
            format!("[{}:{}] {}", file, line, target)
        } else {
            format!("[{}:{}] {} - {}", file, line, target, message)
        };

        // Forward to tklog based on level using macros
        match *level {
            tracing::Level::ERROR => tklog::error!("{}", formatted),
            tracing::Level::WARN => tklog::warn!("{}", formatted),
            tracing::Level::INFO => tklog::info!("{}", formatted),
            tracing::Level::DEBUG => tklog::debug!("{}", formatted),
            tracing::Level::TRACE => tklog::trace!("{}", formatted),
        }
    }
}

/// Visitor to extract fields from tracing events.
#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    #[allow(dead_code)]
    fields: Vec<(String, String)>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let name = field.name().to_string();
        let value = format!("{:?}", value);

        if name == "message" {
            self.message = Some(value);
        } else {
            self.fields.push((name, value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        let name = field.name().to_string();
        let value = value.to_string();

        if name == "message" {
            self.message = Some(value);
        } else {
            self.fields.push((name, value));
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.push((field.name().to_string(), value.to_string()));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.push((field.name().to_string(), value.to_string()));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.push((field.name().to_string(), value.to_string()));
    }
}
