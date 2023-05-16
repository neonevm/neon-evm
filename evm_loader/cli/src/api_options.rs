use clap::{crate_description, crate_name, App, Arg, ArgMatches};

pub fn parse<'a>() -> ArgMatches<'a> {
    App::new(crate_name!())
        .about(crate_description!())
        .version(concat!(
            "Neon-API/v",
            env!("CARGO_PKG_VERSION"),
            "-",
            env!("NEON_REVISION")
        ))
        .arg({
            Arg::with_name("host")
                .short("H")
                .long("host")
                .value_name("HOST")
                .takes_value(true)
                .global(true)
                .help("API host")
        })
        .get_matches()
}
