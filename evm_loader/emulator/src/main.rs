mod hamt;
mod solana_backend;
mod account_data;
mod solidity_account;

use crate::solana_backend::SolanaBackend;

use evm::{executor::StackExecutor, ExitReason};
use hex;
use primitive_types::{H160, U256};
use solana_sdk::pubkey::Pubkey;
use std::env;
use std::str::FromStr;

fn main() {
    let args: Vec<String> = env::args().collect();

    let evm_loader = &args[1];
    let contract_id = &args[2];
    let caller_id = &args[3];
    let data = &args[4];

    let evm_loader = Pubkey::from_str(&evm_loader.to_string()).unwrap();
    let contract_id = H160::from_str(&contract_id.to_string()).unwrap();
    let caller_id = H160::from_str(&caller_id.to_string()).unwrap();
    let data = hex::decode(&data.to_string()).unwrap();

    let mut backend = SolanaBackend::new(evm_loader, contract_id, caller_id).unwrap();
    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);

    let (exit_reason, result) = executor.transact_call(
        caller_id,
        contract_id,
        U256::zero(),
        data,
        usize::max_value(),
    );

    println!("Call done");
    println!("{}", match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, logs) = executor.deconstruct();
            backend.apply(applies, logs, false);
            println!("Applies done");
            "succeed"
        }
        ExitReason::Error(_) => "error",
        ExitReason::Revert(_) => "revert",
        ExitReason::Fatal(_) => "fatal",
    });

    println!("{}", &hex::encode(&result));
    if !exit_reason.is_succeed() {
        println!("Not succeed execution");
    }
}
