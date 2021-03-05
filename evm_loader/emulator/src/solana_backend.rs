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
    info,
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

use solana_client::rpc_client::RpcClient;

fn keccak256_digest(data: &[u8]) -> H256 {
    H256::from_slice(Keccak256::digest(&data).as_slice())
}

pub fn solidity_address(key: &Pubkey) -> H160 {
    H256::from_slice(key.as_ref()).into()
}

fn u256_to_h256(value: U256) -> H256 {
    let mut v = vec![0u8; 32];
    value.to_big_endian(&mut v);
    H256::from_slice(&v)
}

pub struct SolanaBackend {
    accounts: Vec<(H160, SolidityAccount)>,
    rpc_client: RpcClient,
    program_id: Pubkey,
    contract_id: H160,
    caller_id: H160,
}

impl SolanaBackend {
    pub fn new(program_id: Pubkey, contract_id: H160, caller_id: H160,) -> Result<Self,ProgramError> {
        println!("backend::new");
        Ok(Self {
            accounts: Vec::new(),
            rpc_client: RpcClient::new("http://localhost:8899".to_string()),
            program_id: program_id,
            contract_id: contract_id,
            caller_id: caller_id,
        })
    }

    fn get_account(&self, address: H160) -> Option<&SolidityAccount> {
        match self.accounts.binary_search_by_key(&address, |v| v.0) {
            Ok(pos) => {
                Ok(pos)
            },
            Err(_) => {
                println!("Not found account for {}", &address.to_string());

                let (solana_address, nonce) = Pubkey::find_program_address(&[address.to_fixed_bytes()], self.program_id);
                
                match self.rpc_client.get_account(&solana_address) {
                    Ok(acc) => {
                        println!("Account found");                        
                        println!("Account data len {}", acc.data.len());
                        println!("Account owner {}", acc.owner.to_string());

                        self.accounts.push((address, SolidityAccount::new(acc.data, acc.lamports)));                        
                        self.accounts.sort_by_key(|v| v.0);

                        self.accounts.binary_search_by_key(&address, |v| v.0)
                    },
                    Err(_) => {
                        println!("Account not found");
                        None
                    }
                };
            },
        }
    }

    fn get_account_mut(&mut self, address: H160) -> Option<&mut SolidityAccount> {
        match self.accounts.binary_search_by_key(&address, |v| v.0) {
            Ok(pos) => {
                Ok(pos).as_mut()
            },
            Err(_) => {
                println!(&("Not found account for ".to_owned() + &address.to_string()));

                let (solana_address, nonce) = Pubkey::find_program_address(&[address.to_fixed_bytes()], self.program_id);
                
                match self.rpc_client.get_account(&solana_address) {
                    Ok(acc) => {
                        println!("Account found");                        
                        println!("Account data len {}", acc.data.len());
                        println!("Account owner {}", acc.owner.to_string());

                        self.accounts.push((address, SolidityAccount::new(acc.data, acc.lamports)));                        
                        self.accounts.sort_by_key(|v| v.0);

                        self.accounts.binary_search_by_key(&address, |v| v.0).as_mut()
                    },
                    Err(_) => {
                        println!("Account not found");
                        None
                    }
                };
            },
        }
    }

    fn is_solana_address(&self, code_address: &H160) -> bool {
        *code_address == Self::system_account()
    }

    pub fn system_account() -> H160 {
        H160::from_slice(&[0xffu8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8])
    }

    pub fn apply<A, I, L>(&mut self, values: A, logs: L, delete_empty: bool)
            where
                A: IntoIterator<Item=Apply<I>>,
                I: IntoIterator<Item=(H256, H256)>,
                L: IntoIterator<Item=Log>,
    {             
        for apply in values {
            match apply {
                Apply::Modify {address, basic, code, storage, reset_storage} => {
                    println!("Modify: {} {} {}", address.to_string(), basic.nonce.as_u64(), basic.balance.as_u64());
                },
                Apply::Delete {address: addr} => {
                    println!("Delete: {}", addr.to_string());
                },
            }
        };
    }
}

impl Backend for SolanaBackend {
    fn gas_price(&self) -> U256 { U256::zero() }
    fn origin(&self) -> H160 { self.contract_id }
    fn block_hash(&self, _number: U256) -> H256 { H256::default() }
    fn block_number(&self) -> U256 {
        let slot = match self.rpc_client.get_slot(){
            Ok(slot) => {
                println!("Got slot");                
                println!("Slot {}", slot);    
                slot
            },
            Err(_) => {
                println!("Get slot error");    
                0
            }
        };
        
        slot.into()
    }
    fn block_coinbase(&self) -> H160 { H160::default() }
    fn block_timestamp(&self) -> U256 {
        let slot = match self.rpc_client.get_slot() {
            Ok(slot) => {
                println!("Got slot");                
                println!("Slot {}", slot);    
                slot
            },
            Err(_) => {
                println!("Get slot error");    
                0
            }
        };
    
        let timestamp = match self.rpc_client.get_block_time(slot) {
            Ok(timestamp) => {
                println!("Got timestamp");                
                println!("timestamp {}", timestamp);
                timestamp
            },
            Err(_) => {
                println!("Get timestamp error");    
                0
            }
        };

        timestamp.into()
    }
    fn block_difficulty(&self) -> U256 { U256::zero() }
    fn block_gas_limit(&self) -> U256 { U256::zero() }
    fn chain_id(&self) -> U256 { U256::zero() }

    fn exists(&self, address: H160) -> bool {
        self.get_account(address).map_or(false, |_| true)
    }
    fn basic(&self, address: H160) -> Basic {
        match self.get_account(address) {
            None => Basic{balance: U256::zero(), nonce: U256::zero()},
            Some(acc) => Basic{
                balance: (**acc.lamports.borrow()).into(),
                nonce: U256::from(acc.account_data.trx_count),
            },
        }
    }
    fn code_hash(&self, address: H160) -> H256 {
        self.get_account(address).map_or_else(
                || keccak256_digest(&[]), 
                |acc| acc.code(|d| {println!(&hex::encode(&d[0..32])); keccak256_digest(d)})
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
    fn create(&self, _scheme: &CreateScheme, _address: &H160) {
        if let CreateScheme::Create2 {caller, code_hash, salt} = _scheme {
            println!("CreateScheme2 {} from {} {} {}", &hex::encode(_address), &hex::encode(caller) ,&hex::encode(code_hash), &hex::encode(salt));
        } else {
            println!("Call create");
        }
    /*    let account = if let CreateScheme::Create2{salt,..} = scheme
                {Pubkey::new(&salt.to_fixed_bytes())} else {Pubkey::default()};
        self.add_alias(address, &account);*/
    }
};


