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

#[cfg(target_os = "solana")]
macro_rules! log_msg {
    ($($arg:tt)*) => (solana_program::msg!($($arg)*));
}

#[cfg(not(target_os = "solana"))]
macro_rules! log_msg {
    ($($arg:tt)*) => (log::info!($($arg)*));
}

#[inline]
pub fn log_data(data: &[&[u8]]) {
    #[cfg(target_os = "solana")]
    solana_program::log::sol_log_data(data);

    #[cfg(not(target_os = "solana"))]
    {
        let mut messages: Vec<String> = Vec::new();

        for f in data {
            if let Ok(str) = String::from_utf8(f.to_vec()) {
                messages.push(str);
            } else {
                messages.push(hex::encode(f));
            }
        }

        log::info!("Program Data: {}", messages.join(" "));
    }
}
