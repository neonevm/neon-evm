use std::mem::size_of;

use evm::U256;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

use crate::account::{AccountData, ether_contract, Packable};
use crate::account::ether_contract::{DataV1, ExtensionV1};
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use crate::hamt::Hamt;

const TAG_SIZE: usize = size_of::<u8>();
const STORAGE_ENTRY_SIZE: usize = size_of::<U256>();
const CONTRACT_STORAGE_SIZE: usize =
    STORAGE_ENTRY_SIZE * STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize;

enum AccountIndexes {
    EthereumContract,
}

fn check_account_ownership(program_id: &Pubkey, account: &AccountInfo) -> ProgramResult {
    if account.owner == program_id {
        return Ok(());
    }

    msg!(
        "Fail: The owner of tne account being processed is {}, but it must be {}.",
        account.owner,
        program_id,
    );

    Err(ProgramError::IncorrectProgramId)
}

fn extract_contract_storage(hamt: &Hamt) -> Vec<u8> {
    fn write_value(index: usize, value: &U256, contract_storage: &mut [u8]) {
        assert!(index < STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize);
        let offset = index * STORAGE_ENTRY_SIZE;
        value.to_big_endian(&mut contract_storage[offset..offset + 32]);
    }

    const MAX_ELEMENTS_FOR_ITERATION: usize =
        size_of::<u32>() * STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize * 8;
    let mut contract_storage = vec![0u8; CONTRACT_STORAGE_SIZE];
    if hamt.last_used() as usize > MAX_ELEMENTS_FOR_ITERATION {
        for index in 0..STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize {
            if let Some(value) = hamt.find(index.into()) {
                write_value(index, &value, &mut contract_storage);
            }
        }
    } else {
        for (key, value) in hamt.iter() {
            if key.as_u32() < STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT {
                write_value(key.as_usize(), &value, &mut contract_storage);
            }
        }
    }
    contract_storage
}

fn convert_to_v2<'a>(
    mut data: Vec<u8>,
    ethereum_contract_v1: &AccountData<'a, DataV1, ExtensionV1<'a>>,
) -> Vec<u8> {
    const DATA_END: usize = TAG_SIZE + DataV1::SIZE;

    let contract_storage = extract_contract_storage(&ethereum_contract_v1.extension.storage);
    let code_size = ethereum_contract_v1.code_size as usize;
    let valids_size = code_size / 8 + 1;
    let contract_storage_start = DATA_END + code_size + valids_size;

    // Trim or extend storage to `STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT` elements:
    data.resize(contract_storage_start + CONTRACT_STORAGE_SIZE, 0);
    // Overwrite storage with the first elements:
    data.splice(
        contract_storage_start..contract_storage_start + CONTRACT_STORAGE_SIZE,
        contract_storage,
    );

    // Insert `generation` field:
    data.splice(DATA_END..DATA_END, 0u32.to_le_bytes());

    // Update data tag:
    data[0] = ether_contract::Data::TAG;

    data
}

/// Processes the conversion of a data account from V1 to V2.
pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction_data: &[u8],
) -> ProgramResult {
    msg!("Instruction: ConvertStorageAccountFromV1ToV2");

    let account_info = &accounts[AccountIndexes::EthereumContract as usize];

    check_account_ownership(program_id, account_info)?;

    let data = account_info.data.borrow().to_vec();
    let ethereum_contract_v1 =
        AccountData::<ether_contract::DataV1, ether_contract::ExtensionV1>::from_account(
            program_id,
            account_info,
        )?;

    let data = convert_to_v2(data, &ethereum_contract_v1);

    account_info.realloc(data.len(), false)?;
    account_info.data.borrow_mut().copy_from_slice(&data);

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::BorrowMut;
    use std::cell::{RefCell, RefMut};
    use std::collections::HashSet;

    use evm::U256;
    use solana_program::account_info::AccountInfo;
    use solana_program::entrypoint::ProgramResult;
    use solana_program::pubkey::Pubkey;

    use crate::account::{AccountData, EthereumContract, Packable};
    use crate::account::ether_contract::{DataV1, ExtensionV1};
    use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
    use crate::hamt::Hamt;
    use crate::instruction::convert_data_account_from_v1_to_v2::{
        check_account_ownership,
        CONTRACT_STORAGE_SIZE,
        convert_to_v2,
        extract_contract_storage,
        STORAGE_ENTRY_SIZE,
        TAG_SIZE,
    };

    const VALUE_MULTIPLICATOR: u32 = 100;

    #[test]
    fn test_check_account_ownership() {
        let pubkey = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut lamports = 1u64;
        let dummy_account = AccountInfo::new(
            &pubkey,
            false,
            false,
            &mut lamports,
            &mut [],
            &owner,
            false,
            0,
        );

        assert!(check_account_ownership(&owner, &dummy_account).is_ok());
        assert!(check_account_ownership(&pubkey, &dummy_account).is_err());
    }

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
        let mut data = vec![0u8; TAG_SIZE + DataV1::SIZE];
        let owner = Pubkey::new_unique();
        let pubkey = Pubkey::new_unique();
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

        let mut lamports = 1u64;
        let account_info = AccountInfo::new(
            &pubkey,
            false,
            false,
            &mut lamports,
            &mut data,
            &owner,
            false,
            0,
        );

        let mut converted = {
            let data = account_info.data.borrow().to_vec();
            let ethereum_contract_v1 =
                AccountData::<DataV1, ExtensionV1>::from_account(
                    &owner,
                    &account_info,
                )?;

            convert_to_v2(data, &ethereum_contract_v1)
        };

        account_info.data.replace(converted.borrow_mut());

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
        assert_eq!(contract_storage.len(), CONTRACT_STORAGE_SIZE);

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
