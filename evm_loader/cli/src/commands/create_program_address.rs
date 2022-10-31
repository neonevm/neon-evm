use evm_loader::types::Address;

use crate::{
    Config,
};

pub fn execute (
    config: &Config,
    ether_address: &Address,
) {
    let (solana_address, nonce) = ether_address.find_solana_address(&config.evm_loader);
    println!("{} {}", solana_address, nonce);
}

