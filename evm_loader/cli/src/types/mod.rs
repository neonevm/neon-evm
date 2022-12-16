// ec module was copied from OpenEthereum and is kept as close
// as possible to original
#[rustfmt::skip]
#[allow(clippy::all)]
pub mod ec;
mod primitive;

pub use primitive::*;

use evm_loader::H256;
// use parity_bytes::ToPretty;

type Bytes = Vec<u8>;
#[derive(Debug, Clone)]
pub struct TxMeta<T> {
    pub slot: u64,
    pub eth_signature: H256,
    pub sol_signature: Vec<u8>,
    pub value: T,
}

impl<T> TxMeta<T> {
    pub fn split(self) -> (TxMeta<()>, T) {
        let new_meta = TxMeta {
            slot: self.slot,
            eth_signature: self.eth_signature,
            sol_signature: self.sol_signature,
            value: (),
        };

        (new_meta, self.value)
    }

    pub fn wrap<U>(self, new_value: U) -> TxMeta<U> {
        TxMeta {
            slot: self.slot,
            eth_signature: self.eth_signature,
            sol_signature: self.sol_signature,
            value: new_value,
        }
    }
}


#[derive(Debug, Default, PartialEq, serde::Deserialize)]
pub struct EthCallObject {
    pub from: Option<H160T>,
    pub to: Option<H160T>,
    pub gas: Option<U256T>,
    pub gasprice: Option<U256T>,
    pub value: Option<U256T>,
    pub data: Option<Bytes>,
}


