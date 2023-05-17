#[cfg(target_os = "solana")]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => {};
}

#[cfg(all(not(target_os = "solana"), feature = "log"))]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => { log::debug!( $( $args ),* ) }
}

#[cfg(all(not(target_os = "solana"), not(feature = "log")))]
#[macro_export]
macro_rules! debug_print {
    ($( $args:expr ),*) => {};
}
