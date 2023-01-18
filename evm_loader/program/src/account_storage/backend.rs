use crate::account::EthereumAccount;
use crate::account_storage::{AccountStorage, ProgramAccountStorage};
use crate::config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT;
use crate::executor::{OwnedAccountInfo, OwnedAccountInfoPartial};
use evm::{H160, H256, U256};
use solana_program::{
    pubkey::Pubkey,
    sysvar::slot_hashes::{self, SlotHashes},
};

use super::generate_fake_block_hash;

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
        let slot_hashes_account = self
            .solana_accounts
            .get(&slot_hashes::ID)
            .unwrap_or_else(|| {
                panic!(
                    "Trying to get slot hash info without providing sysvar account: {}",
                    slot_hashes::ID
                )
            });
        let slot_hashes: SlotHashes = slot_hashes_account
            .deserialize_data()
            .unwrap_or_else(|e| panic!("Error {e} while deserializing sysvar {}", slot_hashes::ID));
        let slot = self.clock.slot - 1 - number.as_u64();
        slot_hashes
            .get(&slot)
            .map_or_else(|| generate_fake_block_hash(slot), |x| x.to_bytes())
            .into()
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
        self.ethereum_account(address)
            .map_or(0, |a| a.code_size as usize)
    }

    fn code_hash(&self, address: &H160) -> H256 {
        self.ethereum_account(address)
            .and_then(EthereumAccount::contract_data)
            .map_or_else(H256::zero, |contract| {
                crate::utils::keccak256_h256(&contract.code())
            })
    }

    fn code(&self, address: &H160) -> Vec<u8> {
        self.ethereum_account(address)
            .and_then(EthereumAccount::contract_data)
            .map_or_else(Vec::new, |contract| contract.code().to_vec())
    }

    fn valids(&self, address: &H160) -> Vec<u8> {
        self.ethereum_account(address)
            .and_then(EthereumAccount::contract_data)
            .map_or_else(Vec::new, |contract| contract.valids().to_vec())
    }

    fn generation(&self, address: &H160) -> u32 {
        self.ethereum_account(address)
            .map_or(0_u32, |c| c.generation)
    }

    fn storage(&self, address: &H160, index: &U256) -> U256 {
        if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            return self
                .ethereum_account(address)
                .and_then(EthereumAccount::contract_data)
                .map_or_else(U256::zero, |contract| {
                    U256::from_big_endian(&contract.storage()[index..index + 32])
                });
        }

        #[allow(clippy::cast_possible_truncation)]
        let subindex = (*index & U256::from(0xFF)).as_u64() as u8;
        let index = *index & !U256::from(0xFF);

        self.ethereum_storage(*address, index)
            .map_or_else(U256::zero, |a| a.get(subindex))
    }

    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo {
        let info = self.solana_accounts[address];
        OwnedAccountInfo::from_account_info(info)
    }

    fn clone_solana_account_partial(
        &self,
        address: &Pubkey,
        offset: usize,
        len: usize,
    ) -> Option<OwnedAccountInfoPartial> {
        let info = self.solana_accounts[address];
        OwnedAccountInfoPartial::from_account_info(info, offset, len)
    }

    fn solana_account_space(&self, address: &H160) -> Option<usize> {
        self.ethereum_account(address)
            .map(|account| account.info.data_len())
            .or_else(|| {
                let (solana_address, _bump_seed) = self.calc_solana_address(address);
                self.solana_accounts
                    .get(&solana_address)
                    .filter(|info| !solana_program::system_program::check_id(info.owner))
                    .map(|info| {
                        assert_eq!(info.owner, self.program_id());
                        info.data_len()
                    })
            })
    }

    fn solana_address(&self, address: &H160) -> (Pubkey, u8) {
        self.ethereum_accounts.get(address).map_or_else(
            || self.calc_solana_address(address),
            |a| (*a.info.key, a.bump_seed),
        )
    }

    fn chain_id(&self) -> u64 {
        crate::config::CHAIN_ID
    }
}
