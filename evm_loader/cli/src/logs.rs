use std::sync::Mutex;

use clap::ArgMatches;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LogRecord {
    message: String,
    source: String,
    level: &'static str,
}

pub static CONTEXT: Mutex<Vec<LogRecord>> = Mutex::new(Vec::new());

pub fn init(options: &ArgMatches) -> Result<(), log::SetLoggerError> {
    let log_level: log::LevelFilter =
        options
            .value_of("loglevel")
            .map_or(log::LevelFilter::Trace, |ll| {
                match ll.to_ascii_lowercase().as_str() {
                    "off" => log::LevelFilter::Off,
                    "error" => log::LevelFilter::Error,
                    "warn" => log::LevelFilter::Warn,
                    "info" => log::LevelFilter::Info,
                    "debug" => log::LevelFilter::Debug,
                    _ => log::LevelFilter::Trace,
                }
            });

    fern::Dispatch::new()
        .filter(move |metadata| {
            const MODULES: [&str; 3] = ["neon_cli", "neon_lib", "evm_loader"];

            let target = metadata.target();
            for module in MODULES {
                if target.starts_with(module) {
                    return metadata.level().to_level_filter() <= log_level;
                }
            }

            metadata.level() <= log::Level::Error
        })
        .chain(fern::Output::call(|record| {
            let file: &str = record.file().unwrap_or("undefined");
            let line: u32 = record.line().unwrap_or(0);

            let mut context = CONTEXT.lock().unwrap();
            context.push(LogRecord {
                message: record.args().to_string(),
                source: format!("{file}:{line}"),
                level: record.metadata().level().as_str(),
            });
        }))
        .apply()
}
