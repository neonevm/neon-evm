use std::convert::TryInto;
use evm::{H160, H256, U256};
use solana_program::{
    pubkey::Pubkey,
    sysvar::recent_blockhashes
};
use crate::account::{EthereumContract, EthereumStorage, ACCOUNT_SEED_VERSION};
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use crate::executor::OwnedAccountInfo;

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
        self.clock.unix_timestamp.into()
    }

    fn block_hash(&self, number: U256) -> H256 {
        if let Some(account) = self.solana_accounts.get(&recent_blockhashes::ID) {
            let slot_hash_data = account.data.borrow();
            let clock_slot = self.clock.slot;
            if number >= clock_slot.into() {
                return H256::default();
            }
            let offset: usize = (8 + (clock_slot - 1 - number.as_u64()) * 40).try_into().unwrap();
            if offset + 32 > slot_hash_data.len() {
                return H256::default();
            }
            return H256::from_slice(&slot_hash_data[offset..][..32]);
        }
        panic!("Trying to get blockhash info without providing sysvar account: {}", recent_blockhashes::ID);
    }

    fn exists(&self, address: &H160) -> bool {
        self.ethereum_accounts.contains_key(address)
    }

    fn nonce(&self, address: &H160) -> U256 {
        self.ethereum_account(address)
            .map_or(0_u64, |a| a.trx_count)
            .into()
    }

    fn balance(&self, address: &H160) -> U256 {
        self.ethereum_account(address)
            .map_or_else(U256::zero, |a| a.balance)
    }

    fn code_size(&self, address: &H160) -> usize {
        self.ethereum_contract(address)
            .map_or(0_u32, |c| c.code_size)
            .try_into()
            .expect("usize is 8 bytes")
    }

    fn code_hash(&self, address: &H160) -> H256 {
        self.ethereum_contract(address)
            .map(|c| &*c.extension.code)
            .map_or_else(H256::zero, crate::utils::keccak256_h256)
    }

    fn code(&self, address: &H160) -> Vec<u8> {
        self.ethereum_contract(address)
            .map(|c| &c.extension.code)
            .map_or_else(Vec::new, |code| code.to_vec())
    }

    fn valids(&self, address: &H160) -> Vec<u8> {
        self.ethereum_contract(address)
            .map(|c| &c.extension.valids)
            .map_or_else(Vec::new, |valids| valids.to_vec())
    }

    fn generation(&self, address: &H160) -> u32 {
        self.ethereum_contract(address)
            .map_or(0_u32, |c| c.generation)
    }

    fn storage(&self, address: &H160, index: &U256) -> U256 {
        if *index < U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            return self.ethereum_contract(address)
                .map(|c| &c.extension.storage[index..index+32])
                .map_or_else(U256::zero, U256::from_big_endian);
        }

        let (solana_address, _) = self.get_storage_address(address, index);
        let account = self.solana_accounts.get(&solana_address)
            .unwrap_or_else(|| panic!("Account {} - storage account not found", solana_address));

        if account.owner == self.program_id {
            let storage = EthereumStorage::from_account(self.program_id, account).unwrap();
            return storage.value
        }

        if solana_program::system_program::check_id(account.owner) {
            return U256::zero()
        }

        panic!("Account {} - expected system or program owned", solana_address);
    }

    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        let info = self.solana_accounts[address];
        OwnedAccountInfo::from_account_info(info)
    }

    fn solana_accounts_space(&self, address: &H160) -> (usize, usize) {
        let account_space = {
            self.ethereum_account(address)
                .map_or(0, |a| a.info.data_len())
        };

        let contract_space = {
            self.ethereum_contract(address)
                .map_or(0, |a| {
                    EthereumContract::SIZE
                        + a.extension.code.len()
                        + a.extension.valids.len()
                        + a.extension.storage.len()
                })
        };

        (account_space, contract_space)
    }

    fn solana_address(&self, address: &H160) -> (Pubkey, u8) {
        use super::Account;

        #[allow(clippy::match_same_arms)]
        match self.ethereum_accounts.get(address) {
            Some(Account::User(a)) => (*a.info.key, a.bump_seed),
            Some(Account::Contract(a, _)) => (*a.info.key, a.bump_seed),
            None => Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], address.as_bytes()], self.program_id)
        }
    }

    fn chain_id(&self) -> u64 {
        crate::config::CHAIN_ID
    }
}