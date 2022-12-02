use evm_loader::{H160};

use crate::{
    Config,
    account_storage::make_solana_program_address,
};

pub fn execute (
    config: &Config,
    ether_address: &H160,
) {
    let (solana_address, nonce) = make_solana_program_address(ether_address, &config.evm_loader);
    println!("{} {}", solana_address, nonce);
}

