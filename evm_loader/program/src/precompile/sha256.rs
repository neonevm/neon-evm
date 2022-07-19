use std::convert::Infallible;

use evm::{Capture, ExitReason};

#[must_use]
pub fn sha256(
    input: &[u8],
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    use solana_program::hash::hash as sha256_digest;
    debug_print!("sha256");

    let hash = sha256_digest(input);

    debug_print!("{}", &hex::encode(hash.to_bytes()));

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        hash.to_bytes().to_vec(),
    ))
}