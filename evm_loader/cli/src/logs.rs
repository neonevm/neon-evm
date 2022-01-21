use serde::{ Deserialize, Serialize };
use fern::{ Dispatch };

#[derive(Deserialize,Serialize)]
#[derive(Default)]
pub struct LogContext {
    req_id: String,
}


const LOG_MODULES: [&str; 2] = [
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
                "{:23} {:>8} {:>6}:{:10} {:>15}:{:30} {} {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.level(),
                "NA",
                "Undefined",
                "Emulator",
                record.target(),
                serde_json::to_string(&context).unwrap(),
                message
            ));
        })
        .chain(std::io::stderr())
        .apply()
}