use evm::{
    backend::{Basic, Backend, Apply, Log},
    CreateScheme, Capture, Transfer, ExitReason
};
use core::convert::Infallible;
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use solana_sdk::pubkey::Pubkey;

use crate::solidity_account::SolidityAccount;
use std::borrow::Borrow;

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
    pub fn new(program_id: Pubkey, contract_id: H160, caller_id: H160,) -> Result<Self, u8> {
        println!("backend::new");
        Ok(Self {
            accounts: Vec::new(),
            rpc_client: RpcClient::new("http://localhost:8899".to_string()),
            program_id: program_id,
            contract_id: contract_id,
            caller_id: caller_id,
        })
    }

    fn get_account(&mut self, address: H160) -> Option<&SolidityAccount> {
        match self.accounts.binary_search_by_key(&address, |v| v.0) {
            Ok(pos) => {
                Some(&self.accounts[pos].1)
            },
            Err(_) => {
                println!("Not found account for {}", &address.to_string());

                let (solana_address, nonce) = Pubkey::find_program_address(&[&address.to_fixed_bytes()], &self.program_id);
                
                match self.rpc_client.get_account(&solana_address) {
                    Ok(acc) => {
                        println!("Account found");                        
                        println!("Account data len {}", acc.data.len());
                        println!("Account owner {}", acc.owner.to_string());

                        self.accounts.push((address, SolidityAccount::new(acc.data, acc.lamports).unwrap()));                        
                        self.accounts.sort_by_key(|v| v.0);

                        let pos = self.accounts.binary_search_by_key(&address, |v| v.0).unwrap();
                        
                        Some(&self.accounts[pos].1)
                    },
                    Err(_) => {
                        println!("Account not found");
                        None
                    }
                }
            },
        }
    }

    fn get_account_mut(&mut self, address: H160) -> Option<&mut SolidityAccount> {
        match self.accounts.binary_search_by_key(&address, |v| v.0) {
            Ok(pos) => {
                Some(&mut self.accounts[pos].1)
            },
            Err(_) => {
                println!("Not found account for {}", &address.to_string());

                let (solana_address, nonce) = Pubkey::find_program_address(&[&address.to_fixed_bytes()], &self.program_id);
                
                match self.rpc_client.get_account(&solana_address) {
                    Ok(acc) => {
                        println!("Account found");                        
                        println!("Account data len {}", acc.data.len());
                        println!("Account owner {}", acc.owner.to_string());

                        self.accounts.push((address, SolidityAccount::new(acc.data, acc.lamports).unwrap()));                        
                        self.accounts.sort_by_key(|v| v.0);

                        let pos = self.accounts.binary_search_by_key(&address, |v| v.0).unwrap();
                        
                        Some(&mut  self.accounts[pos].1)
                    },
                    Err(_) => {
                        println!("Account not found");
                        None
                    }
                }
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
                balance: (*acc.lamports.borrow()).into(),
                nonce: U256::from(acc.account_data.trx_count),
            },
        }
    }
    fn code_hash(&self, address: H160) -> H256 {
        self.get_account(address).map_or_else(
                || keccak256_digest(&[]), 
                |acc| acc.code(|d| {println!("{}", &hex::encode(&d[0..32])); keccak256_digest(d)})
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
    fn call_inner(&self,
        _code_address: H160,
        _transfer: Option<Transfer>,
        _input: Vec<u8>,
        _target_gas: Option<usize>,
        _is_static: bool,
        _take_l64: bool,
        _take_stipend: bool,
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {

        return None;
    //     if !self.is_solana_address(&code_address) {
    //         return None;
    //     }

    //     info!("Call inner");
    //     info!(&code_address.to_string());
    //     info!(&hex::encode(&input));

    //     let (cmd, input) = input.split_at(1);
    //     match cmd[0] {
    //         0 => {
    //             let (program_id, input) = input.split_at(32);
    //             let program_id = Pubkey::new(program_id);
        
    //             let (acc_length, input) = input.split_at(2);
    //             let acc_length = acc_length.try_into().ok().map(u16::from_be_bytes).unwrap();
                
    //             let mut accounts = Vec::new();
    //             for i in 0..acc_length {
    //                 use arrayref::{array_ref, array_refs};
    //                 let data = array_ref![input, 35*i as usize, 35];
    //                 let (translate, signer, writable, pubkey) = array_refs![data, 1, 1, 1, 32];
    //                 let pubkey = if translate[0] != 0 {
    //                     let account = self.get_account(H160::from_slice(&pubkey[12..]));
    //                     if let Some(account) = account {
    //                         account.account_info.key.clone()
    //                     } else {
    //                         return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
    //                     }
    //                 } else {
    //                     Pubkey::new(pubkey)
    //                 };
    //                 accounts.push(AccountMeta {
    //                     is_signer: signer[0] != 0,
    //                     is_writable: writable[0] != 0,
    //                     pubkey: pubkey,
    //                 });
    //                 info!(&format!("Acc: {}", pubkey));
    //             };
        
    //             let (_, input) = input.split_at(35 * acc_length as usize);
    //             info!(&hex::encode(&input));

    //             let contract = self.get_account_by_index(0).unwrap();   // do_call already check existence of Ethereum account with such index
    //             let contract_seeds = [contract.account_data.ether.as_bytes(), &[contract.account_data.nonce]];

    //             info!("account_infos");
    //             for info in self.account_infos {
    //                 info!(&format!("  {}", info.key));
    //             };
    //             let result : solana_sdk::entrypoint::ProgramResult;
    //             match self.get_account_by_index(1){
    //                 Some(inner) => {
    //                     let sender = self.get_account_by_index(1).unwrap();   // do_call already check existence of Ethereum account with such index
    //                     let sender_seeds = [sender.account_data.ether.as_bytes(), &[sender.account_data.nonce]];
    //                      result = invoke_signed(
    //                         &Instruction{program_id, accounts: accounts, data: input.to_vec()},
    //                         &self.account_infos, &[&sender_seeds[..], &contract_seeds[..]]
    //                     );

    //                 }
    //                 None => {
    //                     result = invoke_signed(
    //                         &Instruction{program_id, accounts: accounts, data: input.to_vec()},
    //                         &self.account_infos, &[&contract_seeds[..]]
    //                     );
    //                 }
    //             }
    //             if let Err(err) = result {
    //                 info!(&format!("result: {}", err));
    //                 return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
    //             };
    //             return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Stopped), Vec::new())));
    //         },
    //         1 => {
    //             use arrayref::{array_ref, array_refs};
    //             let data = array_ref![input, 0, 66];
    //             let (tr_base, tr_owner, base, owner) = array_refs![data, 1, 1, 32, 32];

    //             let base = if tr_base[0] != 0 {
    //                 let account = self.get_account(H160::from_slice(&base[12..]));
    //                 if let Some(account) = account {account.account_info.key.clone()}
    //                 else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));}
    //             } else {Pubkey::new(base)};

    //             let owner = if tr_owner[0] != 0 {
    //                 let account = self.get_account(H160::from_slice(&owner[12..]));
    //                 if let Some(account) = account {account.account_info.key.clone()}
    //                 else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));}
    //             } else {Pubkey::new(owner)};

    //             let (_, seed) = input.split_at(66);
    //             let seed = if let Ok(seed) = std::str::from_utf8(&seed) {seed}
    //             else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

    //             let pubkey = if let Ok(pubkey) = Pubkey::create_with_seed(&base, seed.into(), &owner) {pubkey}
    //             else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

    //             info!(&format!("result: {}", &hex::encode(pubkey.as_ref())));
    //             return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), pubkey.as_ref().to_vec())));
    //         },
    //         _ => {
    //             return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
    //         }
    //     }
    }
}
