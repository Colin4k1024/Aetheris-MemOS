//! Tracing layer that forwards logs to tklog.
//!
//! This module provides a tracing layer that forwards all log events to tklog,
//! enabling tklog's features like file rotation, compression, and structured logging.

use std::collections::HashMap;
use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
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

impl<S> Layer<S> for TklogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: Context<'_, S>,
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

        // Extract span fields (tenant_id, user_id, agent_id, etc.)
        let mut span_fields = HashMap::new();
        if let Some(span) = ctx.lookup_current() {
            let extensions = span.extensions();
            if let Some(fields) = extensions.get::<SpanFields>() {
                span_fields = fields.0.clone();
            }
        }

        // Format the log message with span context
        let context_info = if !span_fields.is_empty() {
            let mut parts = Vec::new();
            if let Some(tenant_id) = span_fields.get("tenant_id") {
                parts.push(format!("tenant={}", tenant_id));
            }
            if let Some(user_id) = span_fields.get("user_id") {
                parts.push(format!("user={}", user_id));
            }
            if let Some(agent_id) = span_fields.get("agent_id") {
                parts.push(format!("agent={}", agent_id));
            }
            if let Some(session_id) = span_fields.get("session_id") {
                parts.push(format!("session={}", session_id));
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!(" [{}]", parts.join(", "))
            }
        } else {
            String::new()
        };

        // Format the log message
        let formatted = if message.is_empty() {
            format!("[{}:{}] {}{}", file, line, target, context_info)
        } else {
            format!("[{}:{}] {}{} - {}", file, line, target, context_info, message)
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

    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        attrs.record(&mut visitor);

        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            extensions.insert(SpanFields(visitor.fields));
        }
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        let mut visitor = SpanFieldVisitor::default();
        values.record(&mut visitor);

        if let Some(span) = ctx.span(id) {
            let mut extensions = span.extensions_mut();
            if let Some(fields) = extensions.get_mut::<SpanFields>() {
                fields.0.extend(visitor.fields);
            } else {
                extensions.insert(SpanFields(visitor.fields));
            }
        }
    }
}

/// Storage for span fields.
#[derive(Debug, Clone)]
struct SpanFields(HashMap<String, String>);

/// Visitor to extract span fields.
#[derive(Default)]
struct SpanFieldVisitor {
    fields: HashMap<String, String>,
}

impl tracing::field::Visit for SpanFieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(field.name().to_string(), format!("{:?}", value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields.insert(field.name().to_string(), value.to_string());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields.insert(field.name().to_string(), value.to_string());
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
