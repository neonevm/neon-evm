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

    let (solana_url, base_account, evm_loader, contract_id, caller_id, data) = if args.len() == 7 {
        let solana_url = args[1].to_string();
        let base_account = Pubkey::from_str(&args[2].to_string()).unwrap();
        let evm_loader = Pubkey::from_str(&args[3].to_string()).unwrap();
        let contract_id = H160::from_str(&make_clean_hex(&args[4])).unwrap();
        let caller_id = H160::from_str(&make_clean_hex(&args[5])).unwrap();
        let data = hex::decode(&make_clean_hex(&args[6])).unwrap();

        (solana_url, base_account, evm_loader, contract_id, caller_id, data)
    } else if args.len() == 6 {        
        let solana_url = "http://localhost:8899".to_string();
        let base_account = Pubkey::from_str(&args[2].to_string()).unwrap();
        let evm_loader = Pubkey::from_str(&args[2].to_string()).unwrap();
        let contract_id = H160::from_str(&make_clean_hex(&args[3])).unwrap();
        let caller_id = H160::from_str(&make_clean_hex(&args[4])).unwrap();
        let data = hex::decode(&make_clean_hex(&args[5])).unwrap();

        (solana_url, base_account, evm_loader, contract_id, caller_id, data)
    } else {
        eprintln!("{} SOLANA_URL BASE_ACCOUNT EVM_LOADER CONTRACT_ID CALLER_ID DATA", &args[0].to_string());
        eprintln!("or for local cluster");
        eprintln!("{} BASE_ACCOUNt EVM_LOADER CONTRACT_ID CALLER_ID DATA", &args[0].to_string());
        return;
    };

    let mut backend = SolanaBackend::new(solana_url, base_account, evm_loader, contract_id, caller_id).unwrap();
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
    let status = match exit_reason {
        ExitReason::Succeed(_) => {
            let (applies, logs) = executor.deconstruct();
            backend.apply(applies, logs, false);
            eprintln!("Applies done");
            "succeed".to_string()
        }
        ExitReason::Error(_) => "error".to_string(),
        ExitReason::Revert(_) => "revert".to_string(),
        ExitReason::Fatal(_) => "fatal".to_string(),
    };

    eprintln!("{}", &status);
    eprintln!("{}", &hex::encode(&result));

    if !exit_reason.is_succeed() {
        eprintln!("Not succeed execution");
    }

    backend.get_used_accounts(&status, &result);
}

fn make_clean_hex(in_str: &String) -> String {
    if &in_str[..2] == "0x" {
        in_str[2..].to_string()
    } else {        
        in_str.to_string()
    }
}
