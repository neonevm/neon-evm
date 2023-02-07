use std::sync::Mutex;

use serde::{ Deserialize, Serialize };
use clap::ArgMatches;

#[derive(Serialize, Deserialize, Clone)]
pub struct LogRecord {
    message: String,
    source: String,
    level: &'static str,
}

pub static CONTEXT: Mutex<Vec<LogRecord>> = Mutex::new(Vec::new());


pub fn init(options: &ArgMatches) -> Result<(), log::SetLoggerError> {
    let log_level: log::LevelFilter =
        options.value_of("loglevel")
            .map_or(log::LevelFilter::Trace, |ll|
                match ll.to_ascii_lowercase().as_str() {
                    "off"   => log::LevelFilter::Off,
                    "error" => log::LevelFilter::Error,
                    "warn"  => log::LevelFilter::Warn,
                    "info"  => log::LevelFilter::Info,
                    "debug" => log::LevelFilter::Debug,
                    _       => log::LevelFilter::Trace,
                }
            );

    fern::Dispatch::new()
        .filter(move |metadata| {
            let target = metadata.target();

            if target.starts_with("neon_cli") || target.starts_with("evm_loader") {
                return metadata.level().to_level_filter() <= log_level;
            }

            metadata.level() <= log::Level::Warn
        })
        .chain(fern::Output::call(|record| {
            let file: &str = record.file().unwrap_or("undefined");
            let line: u32 = record.line().unwrap_or(0);

            let mut context = CONTEXT.lock().unwrap();
            context.push(LogRecord {
                message: record.args().to_string(),
                source: format!("{file}:{line}"),
                level: record.metadata().level().as_str()
            });
        }))
        .apply()
}
