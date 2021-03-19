
#[cfg(feature = "evm_debug")]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_program::msg!( $( $args ),* ); }
}

#[cfg(not(feature = "evm_debug"))]
macro_rules! debug_print {
    ($( $args:expr ),*) => {}
}