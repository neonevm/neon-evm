use std::mem::size_of;

use evm::U256;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::pubkey::Pubkey;

use crate::account::{AccountData, ether_contract, Packable};
use crate::account::ether_contract::DataV1;
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use crate::hamt::Hamt;

const TAG_SIZE: usize = size_of::<u8>();
const STORAGE_ENTRY_SIZE: usize = size_of::<U256>();
const CONTRACT_STORAGE_SIZE: usize =
    STORAGE_ENTRY_SIZE * STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT as usize;

enum AccountIndexes {
    EthereumContract,
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
        let storage_entries_in_contract_account = U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT);
        for (key, value) in hamt.iter() {
            if key < storage_entries_in_contract_account {
                write_value(key.as_usize(), &value, &mut contract_storage);
            }
        }
    }
    contract_storage
}

fn convert_to_v2<'a>(account_info: &'a AccountInfo<'a>) -> ProgramResult {
    const DATA_END: usize = TAG_SIZE + DataV1::SIZE;
    const GENERATION_FIELD_SIZE: usize = size_of::<u32>();

    let (contract_storage, code_size) = {
        let ethereum_contract_v1 =
            AccountData::<ether_contract::DataV1, ether_contract::ExtensionV1>::from_account(
                account_info.owner,
                account_info,
            )?;
        (
            extract_contract_storage(&ethereum_contract_v1.extension.storage),
            ethereum_contract_v1.code_size as usize,
        )
    };
    let valids_size = code_size / 8 + 1;
    let addition_size = code_size + valids_size;
    let mut contract_storage_start = DATA_END + addition_size;

    let mut data = account_info.data.borrow_mut();

    assert!(data.len() >= contract_storage_start + CONTRACT_STORAGE_SIZE + GENERATION_FIELD_SIZE);

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

    convert_to_v2(&accounts[AccountIndexes::EthereumContract as usize])
}

#[cfg(test)]
mod tests {
    use std::cell::{RefCell, RefMut};
    use std::collections::HashSet;

    use evm::U256;
    use solana_program::account_info::AccountInfo;
    use solana_program::entrypoint::ProgramResult;
    use solana_program::pubkey::Pubkey;

    use crate::account::{EthereumContract, Packable};
    use crate::account::ether_contract::DataV1;
    use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
    use crate::hamt::Hamt;
    use crate::instruction::convert_data_account_from_v1_to_v2::{
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

        convert_to_v2(&account_info)?;

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
