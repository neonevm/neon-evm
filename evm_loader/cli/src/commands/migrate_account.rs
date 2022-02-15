use log::{info};

//use solana_sdk::{
    //instruction::{AccountMeta, Instruction},
    //message::Message,
    //pubkey::Pubkey,
    //transaction::Transaction,
    //system_program,
//};

//use solana_cli::{
//    checks::{check_account_for_fee},
//};

use evm::{H160};

use crate::{
    Config,
    NeonCliResult,
};

/// Executes subcommand `migrate-account`.
#[allow(clippy::unnecessary_wraps)]
pub fn execute(
    config: &Config,
    ether_address: &H160,
) -> NeonCliResult {
    let (ether_pubkey, _nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);

    let ether_account = config.rpc_client.get_account(&ether_pubkey);
    if ether_account.is_err() {
        info!("No ether account {}", ether_address);
    }
    dbg!(ether_account.unwrap());

    Ok(())
}
