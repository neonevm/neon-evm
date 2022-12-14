use std::{ sync::Mutex };
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LogRecord {
    message: String,
    source: String,
    level: &'static str,
}

pub static CONTEXT: Mutex<Vec<LogRecord>> = Mutex::new(Vec::new());

pub fn init(log_level: log::LevelFilter) -> Result<(), log::SetLoggerError> {
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
                source: format!("{}:{}", file, line),
                level: record.metadata().level().as_str()
            });
        }))
        .apply()
}
