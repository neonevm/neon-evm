use crate::solana_backend::AccountStorage;
use evm::{
    backend::{Basic, Backend, Apply, Log},
    CreateScheme, Capture, Transfer, ExitReason
};
use core::convert::Infallible;
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use solana_sdk::{
    account_info::{next_account_info, AccountInfo},
    pubkey::Pubkey,
    program_error::ProgramError,
    sysvar::{clock::Clock, Sysvar},
    instruction::{Instruction, AccountMeta},
};
use std::{
    cell::RefCell,
};

use crate::solidity_account::SolidityAccount;
use crate::account_data::AccountData;
use solana_sdk::program::invoke;
use solana_sdk::program::invoke_signed;
use std::convert::TryInto;
use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};

fn keccak256_digest(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(&data).as_slice())
}

fn u256_to_h256(value: U256) -> H256 {
    let mut v = vec![0u8; 32];
    value.to_big_endian(&mut v);
    H256::from_slice(&v)
}

pub struct ProgramAccountStorage<'a> {
    accounts: Vec<Option<SolidityAccount<'a>>>,
    aliases: RefCell<Vec<(H160, usize)>>,
    clock_account: &'a AccountInfo<'a>,
    account_infos: &'a [AccountInfo<'a>],
}

impl<'a> ProgramAccountStorage<'a> {
    pub fn new(program_id: &Pubkey, account_infos: &'a [AccountInfo<'a>], clock_account: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        debug_print!("backend::new");
        let mut accounts = Vec::with_capacity(account_infos.len());
        let mut aliases = Vec::with_capacity(account_infos.len());

        for (i, account) in (&account_infos).iter().enumerate() {
            debug_print!(&i.to_string());
            if account.owner == program_id {
                let sol_account = SolidityAccount::new(account.key, account.data.clone(), (*account.lamports.borrow()).clone())?;
                aliases.push((sol_account.get_ether(), i));
                accounts.push(Some(sol_account));
            } else {
                accounts.push(None)
            }
        }
        debug_print!("Accounts was read");
        aliases.sort_by_key(|v| v.0);
        Ok(Self {
            accounts: accounts,
            aliases: RefCell::new(aliases),
            clock_account,
            account_infos: account_infos,
        })
    }

    pub fn get_account_by_index(&self, index: usize) -> Option<&SolidityAccount<'a>> {
        if let Some(acc) = &self.accounts[index] {
            Some(&acc)
        } else {
            None
        }
    }

    pub fn get_account_by_index_mut(&mut self, index: usize) -> Option<&SolidityAccount<'a>> {
        if let Some(acc) = &self.accounts[index] {
            Some(&acc)
        } else {
            None
        }
    }

    fn find_account(&self, address: H160) -> Option<usize> {
        let aliases = self.aliases.borrow();
        match aliases.binary_search_by_key(&address, |v| v.0) {
            Ok(pos) => {
                debug_print!(&("Found account for ".to_owned() + &address.to_string() + " on position " + &pos.to_string()));
                Some(aliases[pos].1)
            }
            Err(_) => {
                debug_print!(&("Not found account for ".to_owned() + &address.to_string()));
                None
            }
        }
    }

    fn get_account(&self, address: H160) -> Option<&SolidityAccount<'a>> {
        if let Some(pos) = self.find_account(address) {
            self.accounts[pos].as_ref()
        } else {
            None
        }
    }

    // fn get_account_mut(&mut self, address: H160) -> Option<(&mut SolidityAccount<'a>, usize)> {
    //     if let Some(pos) = self.find_account(address) {
    //         self.accounts[pos].as_mut()
    //     } else {
    //         None
    //     }
    // }

    fn is_solana_address(&self, code_address: &H160) -> bool {
        *code_address == Self::system_account()
    }

    fn is_ecrecover_address(&self, code_address: &H160) -> bool {
        *code_address == Self::system_account_ecrecover()
    }

    pub fn system_account() -> H160 {
        H160::from_slice(&[
            0xffu8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
            0u8, 0u8, 0u8,
        ])
    }

    pub fn system_account_ecrecover() -> H160 {
        H160::from_slice(&[
            0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8,
            0u8, 0u8, 0x01u8,
        ])
    }

    pub fn apply<A, I>(&mut self, values: A, delete_empty: bool, skip_addr: Option<(H160, bool)>) -> Result<(), ProgramError>
    where
        A: IntoIterator<Item = Apply<I>>,
        I: IntoIterator<Item = (H256, H256)>,
    {
        let ether_addr = skip_addr.unwrap_or_else(|| (H160::zero(), true));
        let system_account = Self::system_account();
        let system_account_ecrecover = Self::system_account_ecrecover();

        for apply in values {
            match apply {
                Apply::Modify {address, basic, code, storage, reset_storage} => {
                    if (address == system_account) || (address == system_account_ecrecover) {
                        continue;
                    }
                    if ether_addr.1 != true && address == ether_addr.0 {
                        continue;
                    }
                    if let Some(pos) = self.find_account(address) {
                        let account = self.accounts[pos].as_mut().ok_or_else(|| ProgramError::NotEnoughAccountKeys)?;
                        let account_info = &self.account_infos[pos];
                        account.update(&account_info, address, basic.nonce, basic.balance.as_u64(), &code, storage, reset_storage)?;
                    }
                }
                Apply::Delete { address: _ } => {}
            }
        }

        //for log in logs {};

        Ok(())
    }
}

impl<'a> AccountStorage for ProgramAccountStorage<'a> {
    fn contract_id(&self) -> H160{
        self.aliases.borrow()[1].0
    }

    fn get_account_solana_address(&self, address: H160) -> Option<&Pubkey> {
        match self.get_account(address) {
            Some(account) => {
                Some(account.solana_address)
            },
            None => None,
        }
    }

    fn get_contract_seeds(&self) -> Option<(H160, u8)> {
        match self.get_account_by_index(0) {
            Some(contract) => {
                Some((contract.account_data.ether, contract.account_data.nonce))
            },
            None => None,
        }
    }

    fn get_caller_seeds(&self) -> Option<(H160, u8)> {
        match self.get_account_by_index(1) {
            Some(caller) => {
                Some((caller.account_data.ether, caller.account_data.nonce))
            },
            None => None,
        }
    }

    fn exists(&self, address: H160) -> bool {
        self.get_account(address).map_or(false, |_| true)
    }

    fn basic(&self, address: H160) -> Basic {
        match self.get_account(address) {
            None => Basic{balance: U256::zero(), nonce: U256::zero()},
            Some(acc) => Basic{
                balance: acc.lamports.into(),
                nonce: U256::from(acc.account_data.trx_count),
            },
        }
    }

    fn code_hash(&self, address: H160) -> H256 {
        self.get_account(address).map_or_else(
                || keccak256_digest(&[]), 
                |acc| acc.code(|d| {debug_print!(&hex::encode(&d[0..32])); keccak256_digest(d)})
            )
    }

    fn code_size(&self, address: H160) -> usize {
        self.get_account(address).map_or_else(|| 0, |acc| acc.code(|d| d.len()))
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.get_account(address).map_or_else(|| Vec::new(), |acc| acc.code(|d| d.into()))
    }

    fn storage(&self, address: H160, index: H256) -> H256 {
        match self.get_account(address) {
            None => H256::default(),
            Some(acc) => {
                let index = index.as_fixed_bytes().into();
                let value = acc.storage(|storage| storage.find(index)).unwrap_or_default();
                if let Some(v) = value {u256_to_h256(v)} else {H256::default()}
            },
        }
    }

    fn block_number(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.slot.into()
    }

    fn block_timestamp(&self) -> U256 {
        let clock = &Clock::from_account_info(self.clock_account).unwrap();
        clock.unix_timestamp.into()
    }
}