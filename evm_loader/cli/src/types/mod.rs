// ec module was copied from OpenEthereum and is kept as close
// as possible to original
#[rustfmt::skip]
#[allow(clippy::all)]
pub mod ec;

use evm_loader::H256;
use parity_bytes::ToPretty;

type Bytes = Vec<u8>;
