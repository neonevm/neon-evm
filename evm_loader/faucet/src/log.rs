//! Faucet log module.

use tracing::{subscriber::Subscriber, Event};
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
        let time_now = String::from("timestamp");

        let normalized_meta = event.normalized_metadata();
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());

        let level = &format!("{}", meta.level())[..1];

        let meta = format!(
            "{} {} {}{}{} ",
            time_now,
            level,
            meta.file().unwrap_or(""),
            String::from(":"),
            meta.line().unwrap_or(0),
        );

        write!(writer, "{}", meta)?;
        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)?;

        Ok(())
    }
}
