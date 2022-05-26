use std::mem::size_of;

use evm::U256;
use solana_program::{msg, system_instruction};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::program::invoke;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;

use crate::account::{AccountData, ether_contract, Packable};
use crate::account::ether_contract::DataV1;
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use crate::error::EvmLoaderError;
use crate::hamt::Hamt;
use crate::instruction::storage_to_v2::OPERATOR_PUBKEY;

const TAG_SIZE: usize = size_of::<u8>();
const STORAGE_ENTRY_SIZE: usize = size_of::<U256>();
const CONTRACT_STORAGE_SIZE: usize =
    STORAGE_ENTRY_SIZE * STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize;

enum AccountIndexes {
    FundingAccount,
    SystemProgram,
    EthereumContract,
}

fn extract_contract_storage(hamt: &Hamt) -> Vec<u8> {
    let mut contract_storage = vec![0u8; CONTRACT_STORAGE_SIZE];
    for index in 0..STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize {
        if let Some(value) = hamt.find(index.into()) {
            let offset = index * STORAGE_ENTRY_SIZE;
            value.to_big_endian(&mut contract_storage[offset..offset + 32]);
        }
    }

    contract_storage
}

fn convert_to_v2<'a>(
    account_info: &'a AccountInfo<'a>,
    funding_account: &'a AccountInfo<'a>,
    system_program: &'a AccountInfo<'a>,
) -> ProgramResult {
    const DATA_END: usize = TAG_SIZE + DataV1::SIZE;
    const GENERATION_FIELD_SIZE: usize = size_of::<u32>();

    let data_size = account_info.data.borrow().len();

    let (contract_storage, min_size_needed, addition_size, mut contract_storage_start) = {
        let ethereum_contract_v1 =
            AccountData::<ether_contract::DataV1, ether_contract::ExtensionV1>::from_account(
                account_info.owner,
                account_info,
            )?;

        let code_size = ethereum_contract_v1.code_size as usize;
        let valids_size = code_size / 8 + 1;
        let addition_size = code_size + valids_size;
        let contract_storage_start = DATA_END + addition_size;

        let min_size_needed = contract_storage_start + CONTRACT_STORAGE_SIZE + GENERATION_FIELD_SIZE;
        if data_size < min_size_needed {
            let rent = Rent::get()?;
            let balance_needed = rent.minimum_balance(min_size_needed);
            let balance_diff = balance_needed.saturating_sub(account_info.lamports());
            if balance_diff != 0 {
                invoke(
                    &system_instruction::transfer(
                        funding_account.key,
                        account_info.key,
                        balance_diff,
                    ),
                    &[
                        funding_account.clone(),
                        account_info.clone(),
                        system_program.clone(),
                    ],
                )?;
            }

            account_info.realloc(min_size_needed, false)?;
        }

        let contract_storage = extract_contract_storage(&ethereum_contract_v1.extension.storage);

        (contract_storage, min_size_needed, addition_size, contract_storage_start)
    };

    let mut data = account_info.data.borrow_mut();

    assert!(data.len() >= min_size_needed);

    // Move `code` and `valids` to the new place:
    data.copy_within(DATA_END..DATA_END + addition_size, DATA_END + GENERATION_FIELD_SIZE);
    contract_storage_start += GENERATION_FIELD_SIZE;

    // Write `generation` field:
    data[DATA_END..DATA_END + GENERATION_FIELD_SIZE].copy_from_slice(&0u32.to_le_bytes()[..]);

    // Overwrite storage with the first elements:
    data[contract_storage_start..contract_storage_start + CONTRACT_STORAGE_SIZE]
        .copy_from_slice(&contract_storage);

    // Update data tag:
    data[0] = ether_contract::Data::TAG;

    Ok(())
}

/// Processes the conversion of a data account from V1 to V2.
pub fn process<'a>(
    _program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("Instruction: ConvertStorageAccountFromV1ToV2");

    let funding_account = &accounts[AccountIndexes::FundingAccount as usize];

    if funding_account.key != &OPERATOR_PUBKEY {
        return Err!(
            EvmLoaderError::UnauthorizedOperator.into();
            "Account {} - expected authorized operator",
            funding_account.key
        );
    }

    let system_program = &accounts[AccountIndexes::SystemProgram as usize];
    let account_info = &accounts[AccountIndexes::EthereumContract as usize];

    convert_to_v2(account_info, funding_account, system_program)
}

#[cfg(test)]
mod tests {
    use std::cell::{RefCell, RefMut};
    use std::collections::HashSet;

    use evm::U256;
    use solana_program::account_info::AccountInfo;
    use solana_program::entrypoint::ProgramResult;
    use solana_program::pubkey::Pubkey;
    use solana_program::system_program;

    use crate::account::{EthereumContract, Packable};
    use crate::account::ether_contract::DataV1;
    use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
    use crate::hamt::Hamt;

    use super::{
        CONTRACT_STORAGE_SIZE,
        convert_to_v2,
        extract_contract_storage,
        STORAGE_ENTRY_SIZE,
        TAG_SIZE,
    };

    const VALUE_MULTIPLICATOR: u32 = 100;

    #[test]
    fn test_extract_contract_storage() -> ProgramResult {
        let buffer = RefCell::new(vec![0u8; 100_000]);
        let hamt_data = RefMut::map(buffer.borrow_mut(), |v| &mut v[..]);
        let mut hamt = Hamt::new(hamt_data)?;

        let keys: HashSet<u32> = [0, 1, 2, 3, 63, 64, 640, 1640].iter().cloned().collect();
        for key in &keys {
            hamt.insert(U256::from(*key), U256::from(*key) * VALUE_MULTIPLICATOR)?;
        }

        let contract_storage = extract_contract_storage(&hamt);

        check_contract_storage(&contract_storage, &keys);

        Ok(())
    }

    #[test]
    fn test_conversion() -> ProgramResult {
        fn simple_account<'a>(
            pubkey: &'a Pubkey,
            lamports: &'a mut u64,
            data: &'a mut [u8],
            owner: &'a Pubkey,
        ) -> AccountInfo<'a> {
            AccountInfo::new(
                pubkey,
                false,
                false,
                lamports,
                data,
                owner,
                false,
                0,
            )
        }

        let mut data = vec![0u8; TAG_SIZE + DataV1::SIZE];
        let owner = Pubkey::new_unique();
        let pubkey = Pubkey::new_unique();
        let funding_pubkey = Pubkey::new_unique();
        let code = vec![1u8, 2, 3];
        let valids = vec![0xFFu8; code.len() / 8 + 1];
        let keys: HashSet<u32> = [0, 1, 2, 3, 63, 64, 640, 1640].iter().cloned().collect();

        let hamt_buffer = RefCell::new(vec![0u8; 100_000]);
        {
            let hamt_data = RefMut::map(hamt_buffer.borrow_mut(), |v| &mut v[..]);
            let mut hamt = Hamt::new(hamt_data)?;

            for key in &keys {
                hamt.insert(U256::from(*key), U256::from(*key) * VALUE_MULTIPLICATOR)?;
            }
        }

        let data_v1 = DataV1 {
            owner: owner.clone(),
            code_size: code.len() as u32,
        };

        data[0] = DataV1::TAG;
        data_v1.pack(&mut data[TAG_SIZE..TAG_SIZE + DataV1::SIZE]);
        data.splice(data.len().., code.clone());
        data.splice(data.len().., valids.clone());
        data.splice(data.len().., hamt_buffer.borrow().iter().cloned());

        let mut account_lamports = 1;
        let mut funding_lamports = 1000;
        let mut system_lamports = 1000;

        let system_program_id = system_program::id();

        let account_info = simple_account(&pubkey, &mut account_lamports, &mut data, &owner);
        let funding_account = simple_account(&funding_pubkey, &mut funding_lamports, &mut [], &owner);
        let system_program = simple_account(&system_program_id, &mut system_lamports, &mut [], &system_program_id);

        convert_to_v2(&account_info, &funding_account, &system_program)?;

        let ethereum_contract_v2 = EthereumContract::from_account(
            &owner,
            &account_info,
        )?;

        assert_eq!(ethereum_contract_v2.owner, owner);
        assert_eq!(ethereum_contract_v2.code_size as usize, code.len());
        assert_eq!(ethereum_contract_v2.generation, 0);
        assert_eq!(*ethereum_contract_v2.extension.code, code);
        assert_eq!(*ethereum_contract_v2.extension.valids, valids);

        check_contract_storage(&ethereum_contract_v2.extension.storage, &keys);

        Ok(())
    }

    fn check_contract_storage(contract_storage: &[u8], keys: &HashSet<u32>) {
        assert!(contract_storage.len() >= CONTRACT_STORAGE_SIZE);

        for i in 0..STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT {
            let offset = i as usize * STORAGE_ENTRY_SIZE;
            let value =
                U256::from_big_endian_fast(&contract_storage[offset..offset + STORAGE_ENTRY_SIZE]);

            if keys.contains(&i) {
                assert_eq!(value, U256::from(i * VALUE_MULTIPLICATOR));
            } else {
                assert!(value.is_zero());
            }
        }
    }
}
