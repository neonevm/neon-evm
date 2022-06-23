use std::convert::Infallible;

use evm::{Capture, ExitReason};


#[must_use]
pub fn datacopy(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    debug_print!("datacopy");

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        input.to_vec(),
    ))
}