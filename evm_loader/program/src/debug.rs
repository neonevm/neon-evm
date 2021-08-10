
#[cfg(all(target_arch = "bpf", not(feature = "no-logs")))]
macro_rules! debug_print {
    ($( $args:expr ),*) => { solana_program::msg!( $( $args ),* ) }
}

#[cfg(all(not(target_arch = "bpf"), not(feature = "no-logs")))]
macro_rules! debug_print {
    ($( $args:expr ),*) => { eprintln!( $( $args ),* ) }
}

#[cfg(feature = "no-logs")]
macro_rules! debug_print {
    ($( $args:expr ),*) => {}
}

macro_rules! error_print {
    ($( $args:expr ),*) =>  {
        solana_program::msg!("{}:{}", file!(), line!());
        solana_program::msg!( $( $args ),* )
    }
}
