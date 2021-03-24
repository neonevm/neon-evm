
#[cfg(feature = "evm_debug")]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_sdk::info!( $( $args ),* ) }
}

#[cfg(feature = "default")]
macro_rules! debug_print {
    ($( $args:expr ),*) => { eprintln!( "{}", $( $args ),* ) }
}