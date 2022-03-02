
#[cfg(all(target_arch = "bpf", not(feature = "no-logs")))]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_program::msg!( $( $args ),* ) }
}

#[cfg(all(not(target_arch = "bpf"), not(feature = "no-logs")))]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => { log::debug!( $( $args ),* ) }
}

#[cfg(feature = "no-logs")]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => {}
}
