
#[cfg(feature = "evm_debug")]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_sdk::info!( $( $args ),* ); }
}

#[cfg(not(feature = "evm_debug"))]
macro_rules! debug_print {
    ($( $args:expr ),*) => {}
}