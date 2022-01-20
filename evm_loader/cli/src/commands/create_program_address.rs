#[allow(clippy::module_name_repetitions)]

use evm::{H160};

use crate::{
    Config,
};

pub fn command_create_program_address (
    config: &Config,
    ether_address: &H160,
) {
    let (solana_address, nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);
    println!("{} {}", solana_address, nonce);
}

