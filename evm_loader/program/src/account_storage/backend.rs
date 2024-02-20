use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;
use crate::error::Result;
use crate::executor::OwnedAccountInfo;
use crate::types::Address;
use ethnum::U256;
use solana_program::account_info::AccountInfo;
use solana_program::{pubkey::Pubkey, rent::Rent, sysvar::slot_hashes};
use std::convert::TryInto;

impl<'a> AccountStorage for ProgramAccountStorage<'a> {
    fn program_id(&self) -> &Pubkey {
        &crate::ID
    }

    fn operator(&self) -> Pubkey {
        self.accounts.operator_key()
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

    fn rent(&self) -> &Rent {
        &self.rent
    }

    fn return_data(&self) -> Option<(Pubkey, Vec<u8>)> {
        solana_program::program::get_return_data()
    }

    fn block_hash(&self, slot: u64) -> [u8; 32] {
        let slot_hashes_account = self.accounts.get(&slot_hashes::ID);
        let slot_hashes_data = slot_hashes_account.data.borrow();

        super::block_hash::find_slot_hash(slot, &slot_hashes_data[..])
    }

    fn nonce(&self, address: Address, chain_id: u64) -> u64 {
        self.balance_account(address, chain_id)
            .map_or(0_u64, |a| a.nonce())
    }

    fn balance(&self, address: Address, chain_id: u64) -> U256 {
        self.balance_account(address, chain_id)
            .map_or(U256::ZERO, |a| a.balance())
    }

    fn is_valid_chain_id(&self, chain_id: u64) -> bool {
        crate::config::CHAIN_ID_LIST
            .binary_search_by_key(&chain_id, |c| c.0)
            .is_ok()
    }

    fn chain_id_to_token(&self, chain_id: u64) -> Pubkey {
        let index = crate::config::CHAIN_ID_LIST
            .binary_search_by_key(&chain_id, |c| c.0)
            .unwrap();

        crate::config::CHAIN_ID_LIST[index].2
    }

    fn default_chain_id(&self) -> u64 {
        crate::config::DEFAULT_CHAIN_ID
    }

    fn contract_chain_id(&self, address: Address) -> Result<u64> {
        let contract = self.contract_account(address)?;
        Ok(contract.chain_id())
    }

    fn contract_pubkey(&self, address: Address) -> (Pubkey, u8) {
        self.keys
            .contract_with_bump_seed(self.program_id(), address)
    }

    fn code_size(&self, address: Address) -> usize {
        self.contract_account(address).map_or(0, |a| a.code_len())
    }

    fn code(&self, address: Address) -> crate::evm::Buffer {
        self.contract_account(address)
            .map_or_else(|_| crate::evm::Buffer::empty(), |a| a.code_buffer())
    }

    fn storage(&self, address: Address, index: U256) -> [u8; 32] {
        if index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT as u64) {
            let index: usize = index.as_usize();
            return self
                .contract_account(address)
                .map(|c| c.storage_value(index))
                .unwrap_or_default();
        }

        let subindex = (index & 0xFF).as_u8();
        let index = index & !U256::new(0xFF);

        self.storage_cell(address, index)
            .map(|a| a.get(subindex))
            .unwrap_or_default()
    }

    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        // This is used to emulate external instruction
        // One of instruction accounts can be operator
        let info = if address == &self.accounts.operator_key() {
            self.accounts.operator_info()
        } else {
            self.accounts.get(address)
        };

        OwnedAccountInfo::from_account_info(self.program_id(), info)
    }

    fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R,
    {
        let info = self.accounts.get(address);
        action(info)
    }
}
