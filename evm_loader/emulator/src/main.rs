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

    let (solana_url, evm_loader, contract_id, caller_id, data) = if args.len() == 6 {
        let solana_url = args[1].to_string();
        let evm_loader = Pubkey::from_str(&args[2].to_string()).unwrap();
        let contract_id = H160::from_str(&args[3].to_string()).unwrap();
        let caller_id = H160::from_str(&args[4].to_string()).unwrap();
        let data = hex::decode(&args[5].to_string()).unwrap();

        (solana_url, evm_loader, contract_id, caller_id, data)
    } else if args.len() == 5 {        
        let solana_url = "http://localhost:8899".to_string();
        let evm_loader = Pubkey::from_str(&args[1].to_string()).unwrap();
        let contract_id = H160::from_str(&args[2].to_string()).unwrap();
        let caller_id = H160::from_str(&args[3].to_string()).unwrap();
        let data = hex::decode(&args[4].to_string()).unwrap();

        (solana_url, evm_loader, contract_id, caller_id, data)
    } else {
        eprintln!("{} SOLANA_URL EVM_LOADER CONTRACT_ID CALLER_ID DATA", &args[0].to_string());
        eprintln!("or for local cluster");
        eprintln!("{} EVM_LOADER CONTRACT_ID CALLER_ID DATA", &args[0].to_string());
        return;
    };

    let mut backend = SolanaBackend::new(solana_url, evm_loader, contract_id, caller_id).unwrap();
    let config = evm::Config::istanbul();
    let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);

    let (exit_reason, result) = executor.transact_call(
        caller_id,
        contract_id,
        U256::zero(),
        data,
        usize::max_value(),
    );

    eprintln!("Call done");
    eprintln!("{}", match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, logs) = executor.deconstruct();
            backend.apply(applies, logs, false);
            eprintln!("Applies done");
            "succeed"
        }
        ExitReason::Error(_) => "error",
        ExitReason::Revert(_) => "revert",
        ExitReason::Fatal(_) => "fatal",
    });

    eprintln!("{}", &hex::encode(&result));
    if !exit_reason.is_succeed() {
        eprintln!("Not succeed execution");
    }

    backend.get_used_accounts();
}
