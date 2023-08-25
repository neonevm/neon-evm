use crate::account::EthereumAccount;
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;
use crate::executor::OwnedAccountInfo;
use crate::types::Address;
use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::{pubkey::Pubkey, sysvar::slot_hashes};
use std::convert::TryInto;

use super::find_slot_hash;

impl<'a> AccountStorage for ProgramAccountStorage<'a> {
    fn neon_token_mint(&self) -> &Pubkey {
        &crate::config::token_mint::ID
    }

    fn program_id(&self) -> &Pubkey {
        self.program_id
    }

    fn operator(&self) -> &Pubkey {
        self.operator
    }

    fn block_number(&self) -> U256 {
        self.clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        self.clock
            .unix_timestamp
            .try_into()
            .expect("Timestamp is positive")
    }

    fn block_hash(&self, slot: u64) -> [u8; 32] {
        let slot_hashes_account = self
            .solana_accounts
            .get(&slot_hashes::ID)
            .unwrap_or_else(|| {
                panic!(
                    "Trying to get slot hash info without providing sysvar account: {}",
                    slot_hashes::ID
                )
            });

        let slot_hashes_data = slot_hashes_account.data.borrow();
        find_slot_hash(slot, &slot_hashes_data[..])
    }

    fn exists(&self, address: &Address) -> bool {
        self.ethereum_accounts.contains_key(address)
    }

    fn nonce(&self, address: &Address) -> u64 {
        self.ethereum_account(address)
            .map_or(0_u64, |a| a.trx_count)
    }

    fn balance(&self, address: &Address) -> U256 {
        self.ethereum_account(address)
            .map_or(U256::ZERO, |a| a.balance)
    }

    fn code_size(&self, address: &Address) -> usize {
        self.ethereum_account(address)
            .map_or(0, |a| a.code_size as usize)
    }

    fn code_hash(&self, address: &Address) -> [u8; 32] {
        use solana_program::keccak::hash;

        // https://eips.ethereum.org/EIPS/eip-1052
        // https://eips.ethereum.org/EIPS/eip-161
        if self.code_size(address) == 0 {
            if self.nonce(address) == 0 && self.balance(address) == 0 {
                // non-existent account
                return <[u8; 32]>::default();
            }

            // account without code
            return hash(&[]).to_bytes();
        }

        self.ethereum_account(address)
            .and_then(EthereumAccount::contract_data)
            .map(|contract| hash(&contract.code()))
            .unwrap_or_default()
            .to_bytes()
    }

    fn code(&self, address: &Address) -> crate::evm::Buffer {
        use crate::evm::Buffer;

        if let Some(account) = self.ethereum_account(address) {
            if account.code_size() == 0 {
                return Buffer::empty();
            }

            unsafe { Buffer::from_account(account.info, account.code_location()) }
        } else {
            Buffer::empty()
        }
    }

    fn generation(&self, address: &Address) -> u32 {
        self.ethereum_account(address)
            .map_or(0_u32, |c| c.generation)
    }

    fn storage(&self, address: &Address, index: &U256) -> [u8; 32] {
        if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            return self
                .ethereum_account(address)
                .and_then(EthereumAccount::contract_data)
                .map(|c| c.storage()[index..index + 32].try_into().unwrap())
                .unwrap_or_default();
        }

        let subindex = (index & 0xFF).as_u8();
        let index = index & !U256::new(0xFF);

        self.ethereum_storage(*address, index)
            .map_or_else(<[u8; 32]>::default, |a| a.get(subindex))
    }

    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        let info = self.solana_accounts[address];
        OwnedAccountInfo::from_account_info(self.program_id, info)
    }

    fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R,
    {
        let info = self.solana_accounts[address];
        action(info)
    }

    fn solana_account_space(&self, address: &Address) -> Option<usize> {
        let (pubkey, _) = self.solana_address(address);
        let info = self.solana_accounts[&pubkey];

        if solana_program::system_program::check_id(info.owner) {
            return None;
        }

        assert_eq!(info.owner, self.program_id);
        Some(info.data_len())
    }

    fn solana_address(&self, address: &Address) -> (Pubkey, u8) {
        self.ethereum_accounts.get(address).map_or_else(
            || address.find_solana_address(self.program_id),
            |a| (*a.info.key, a.bump_seed),
        )
    }

    fn chain_id(&self) -> u64 {
        crate::config::CHAIN_ID
    }
}
