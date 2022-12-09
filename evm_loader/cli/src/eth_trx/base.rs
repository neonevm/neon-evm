use solana_sdk::{message::{Message, SanitizedMessage}};
use crate::{types::TxMeta, config::Config};

use super::EthTrx;

impl<'a> EthTrx<'a> {
    pub fn new(tx_meta: TxMeta<SanitizedMessage>, config: &'a Config) -> Self{
        let (meta, message) = tx_meta.split();
        Self{
            message: message,
            meta: meta,
            config: config,
        }
    }
}