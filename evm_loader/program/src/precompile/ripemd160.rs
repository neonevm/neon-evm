use std::convert::Infallible;

use evm::{Capture, ExitReason};

#[must_use]
pub fn ripemd160(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
    use ripemd::{Digest, Ripemd160};
    debug_print!("ripemd160");

    let mut hasher = Ripemd160::new();
    // process input message
    hasher.update(input);
    // acquire hash digest in the form of GenericArray,
    // which in this case is equivalent to [u8; 20]
    let hash_val = hasher.finalize();

    // transform to [u8; 32]
    let mut result = vec![0_u8; 12];
    result.extend(&hash_val[..]);

    debug_print!("{}", &hex::encode(&result));

    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), result))
}