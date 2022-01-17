//! Faucet version module.

macro_rules! display {
    () => {
        concat!(
            "version ",
            env!("CARGO_PKG_VERSION"),
            "-",
            env!("NEON_REVISION")
        )
    };
}

pub(crate) use display;
