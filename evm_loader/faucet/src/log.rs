//! Faucet log module.

use tracing::{subscriber::Subscriber, Event, Metadata};
use tracing_log::NormalizeEvent;
use tracing_subscriber::{
    fmt::{format, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};

/// Represents custom plain log line format.
pub struct PlainFormat;

impl<S, N> FormatEvent<S, N> for PlainFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> Result<(), std::fmt::Error> {
        let normalized_meta = event.normalized_metadata();
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());

        let timestamp = current_local_timestamp();
        let level = &format!("{}", meta.level())[..1];
        let file_lineno = filename_with_line_number(meta);
        let process = std::process::id();

        let meta = format!("{} {} {} {}", timestamp, level, file_lineno, process);

        write!(writer, "{} ", meta)?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Returns formatted timestamp.
fn current_local_timestamp() -> String {
    let now = chrono::Local::now();
    now.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

/// Returns filename with line number of an event.
fn filename_with_line_number(meta: &Metadata) -> String {
    use std::ffi::OsStr;
    use std::path::Path;

    let filename: &str = meta
        .file()
        .and_then(|filepath| Path::new(filepath).file_name())
        .and_then(OsStr::to_str)
        .unwrap_or("Undefined");
    let line = meta.line().map_or("NA".to_string(), |v| v.to_string());

    format!("{}:{}", filename, line)
}
