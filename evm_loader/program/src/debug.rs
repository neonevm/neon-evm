
#[cfg(all(target_arch = "bpf", feature = "evm_debug"))]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_program::msg!( $( $args ),* ) }
}

#[cfg(all(not(target_arch = "bpf"), feature = "evm_debug"))]
macro_rules! debug_print {
    ($( $args:expr ),*) => { eprintln!( $( $args ),* ) }
}

#[cfg(not(feature = "evm_debug"))]
macro_rules! debug_print {
    ($( $args:expr ),*) => {}
}
