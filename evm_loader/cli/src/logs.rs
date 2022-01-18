use serde::{ Deserialize };
use fern::{ Dispatch };

#[derive(Deserialize)]
#[derive(Default)]
pub struct LogContext {
    req_id: String,
}


const LOG_MODULES: [&'static str; 2] = [
  "neon_cli",
  "neon_cli::account_storage",
];

pub fn init(context: LogContext) -> Result<(), log::SetLoggerError> {
    let mut dispatch: Dispatch =
        fern::Dispatch::new()
            .level(log::LevelFilter::Error);

    for module_name in LOG_MODULES {
        dispatch = dispatch.level_for(module_name, log::LevelFilter::Trace);
    }

    dispatch
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{:23} {:8} {:30} {} {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                record.target(),
                context.req_id,
                message
            ));
        })
        .chain(std::io::stderr())
        .apply()
}