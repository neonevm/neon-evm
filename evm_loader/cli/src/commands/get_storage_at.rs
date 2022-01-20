use std::{
    cell::RefCell,
    rc::Rc
};

use evm::{H160, U256};

use evm_loader::{
    account_data::AccountData,
    solidity_account::SolidityAccount,
};

use crate::{
    account_storage::{
        make_solana_program_address,
        EmulatorAccountStorage,
    },
    Config,
    CommandResult,
};

pub fn command_get_storage_at(
    config: &Config,
    ether_address: &H160,
    index: &U256
) -> CommandResult {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        Some((acc, _balance, code_account)) => {
            let account_data = AccountData::unpack(&acc.data)?;
            let mut code_data = match code_account.as_ref() {
                Some(code) => code.data.clone(),
                None => return Err(format!("Account {:#x} is not code account", ether_address).into()),
            };
            let contract_data = AccountData::unpack(&code_data)?;
            let (solana_address, _solana_nonce) = make_solana_program_address(ether_address, &config.evm_loader);
            let code_data: std::rc::Rc<std::cell::RefCell<&mut [u8]>> = Rc::new(RefCell::new(&mut code_data));
            let solidity_account = SolidityAccount::new(&solana_address, account_data,
                                                        Some((contract_data, code_data)));
            let value = solidity_account.get_storage(index);
            print!("{:#x}", value);
            Ok(())
        },
        None => {
            Err(format!("Account not found {:#x}", ether_address).into())
        }
    }
}

