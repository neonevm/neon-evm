use core::convert::Infallible;
use evm::{
    backend::{Apply, Backend, Basic, Log},
    Capture, CreateScheme, ExitReason, Transfer,
};
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use solana_sdk::pubkey::Pubkey;

use crate::solidity_account::SolidityAccount;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::cell::RefCell; 

use solana_client::rpc_client::RpcClient;

use std::collections::{HashMap, HashSet};

use serde_json::json;
use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize, Debug)]
struct AccountJSON {
    address: String,
    writable: bool,
    new: bool,
}

pub struct SolanaBackend {
    accounts: RefCell<HashMap<H160, SolidityAccount>>,
    new_accounts: RefCell<HashSet<H160>>,
    rpc_client: RpcClient,
    program_id: Pubkey,
    contract_id: H160,
    caller_id: H160,
    base_account: Pubkey,
}

impl SolanaBackend {
    pub fn new(solana_url: String, base_account: Pubkey, program_id: Pubkey, contract_id: H160, caller_id: H160) -> Result<Self, u8> {
        eprintln!("backend::new");
        Ok(Self {
            accounts: RefCell::new(HashMap::new()),
            rpc_client: RpcClient::new(solana_url),
            new_accounts: RefCell::new(HashSet::new()),
            program_id: program_id,
            contract_id: contract_id,
            caller_id: caller_id,
            base_account: base_account,
        })
    }

    fn create_acc_if_not_exists(&self, address: H160) -> bool {
        let mut accounts = self.accounts.borrow_mut(); 
        let mut new_accounts = self.new_accounts.borrow_mut(); 
        if accounts.get(&address).is_none() {

            //let (solana_address, _) = Pubkey::find_program_address(&[&address.to_fixed_bytes()], &self.program_id);
            let seed = bs58::encode(&address.to_fixed_bytes()).into_string();
            let solana_address = Pubkey::create_with_seed(&self.base_account, &seed, &self.program_id).unwrap();

            eprintln!("Not found account for {} => {} (seed {})", &address.to_string(), &solana_address.to_string(), &seed);
            
            match self.rpc_client.get_account(&solana_address) {
                Ok(acc) => {
                    eprintln!("Account found");                        
                    eprintln!("Account data len {}", acc.data.len());
                    eprintln!("Account owner {}", acc.owner.to_string());
                   
                    accounts.insert(address, SolidityAccount::new(acc.data, acc.lamports).unwrap());

                    true
                },
                Err(_) => {
                    eprintln!("Account not found {}", &address.to_string());

                    new_accounts.insert(address);

                    false
                }
            }
        } else {
            true
        }
    }

    pub fn apply<A, I, L>(&mut self, values: A, _logs: L, _delete_empty: bool)
            where
                A: IntoIterator<Item=Apply<I>>,
                I: IntoIterator<Item=(H256, H256)>,
                L: IntoIterator<Item=Log>,
    {             
        let mut accounts = self.accounts.borrow_mut(); 


        for apply in values {
            match apply {
                Apply::Modify {address, basic, code: _, storage: _, reset_storage} => {
                    match accounts.get_mut(&address) {
                        Some(acc) => {
                            *acc.updated.borrow_mut() = true;
                        },
                        None => {
                            eprintln!("Account not found {}", &address.to_string());
                        },
                    }
                    eprintln!("Modify: {} {} {} {}", &address.to_string(), &basic.nonce.as_u64(), &basic.balance.as_u64(), &reset_storage.to_string());
                },
                Apply::Delete {address: addr} => {
                    eprintln!("Delete: {}", addr.to_string());
                },
            }
        };
    }

    pub fn get_used_accounts(&self, status: &String, result: &std::vec::Vec<u8>)
    {          
        let mut arr = Vec::new();    
        
        eprint!("[");
        let accounts = self.accounts.borrow();
        for (address, acc) in accounts.iter() {
            arr.push(AccountJSON{address: "0x".to_string() + &hex::encode(&address.to_fixed_bytes()), writable: acc.updated, new: false});
            eprint!("{{\"address\":\"0x{}\",\"write\":\"{}\"}},", &hex::encode(&address.to_fixed_bytes()), &acc.updated.to_string());
        }
        let new_accounts = self.new_accounts.borrow(); 
        for address in new_accounts.iter() {
            arr.push(AccountJSON{address: "0x".to_string() + &hex::encode(&address.to_fixed_bytes()), writable: false, new: true});
            eprint!("{{\"address\":\"0x{}\",\"new\":\"true\"}},", &hex::encode(&address.to_fixed_bytes()));
        }    
        eprintln!("]");

        let js = json!({"accounts": arr, "result": &hex::encode(&result), "exit_status": &status}).to_string();

        println!("{}", js);
    }
}

impl Backend for SolanaBackend {
    fn gas_price(&self) -> U256 { U256::zero() }
    fn origin(&self) -> H160 { self.contract_id }
    fn block_hash(&self, _number: U256) -> H256 { H256::default() }
    fn block_number(&self) -> U256 {
        let slot = match self.rpc_client.get_slot(){
            Ok(slot) => {
                eprintln!("Got slot");                
                eprintln!("Slot {}", slot);    
                slot
            },
            Err(_) => {
                eprintln!("Get slot error");    
                0
            }
        };
        
        slot.into()
    }
    fn block_coinbase(&self) -> H160 { H160::default() }
    fn block_timestamp(&self) -> U256 {
        let slot = match self.rpc_client.get_slot() {
            Ok(slot) => {
                eprintln!("Got slot");                
                eprintln!("Slot {}", slot);    
                slot
            },
            Err(_) => {
                eprintln!("Get slot error");    
                0
            }
        };
    
        let timestamp = match self.rpc_client.get_block_time(slot) {
            Ok(timestamp) => {
                eprintln!("Got timestamp");                
                eprintln!("timestamp {}", timestamp);
                timestamp
            },
            Err(_) => {
                eprintln!("Get timestamp error");    
                0
            }
        };

        timestamp.into()
    }
    fn block_difficulty(&self) -> U256 { U256::zero() }
    fn block_gas_limit(&self) -> U256 { U256::zero() }
    fn chain_id(&self) -> U256 { U256::zero() }

    fn exists(&self, address: H160) -> bool {
        self.create_acc_if_not_exists(address)
    }
    fn basic(&self, address: H160) -> Basic {
        self.create_acc_if_not_exists(address);
        let accounts = self.accounts.borrow();
        match accounts.get(&address) {
            None => Basic{balance: U256::zero(), nonce: U256::zero()},
            Some(acc) => Basic{
                balance: (*acc.lamports.borrow()).into(),
                nonce: U256::from(acc.account_data.trx_count),
            },
        }
    }
    fn code_hash(&self, address: H160) -> H256 {
        self.create_acc_if_not_exists(address);
        let accounts = self.accounts.borrow();
        match accounts.get(&address) {
            None => keccak256_digest(&[]),
            Some(acc) => {
                acc.code(|d| {eprintln!("{}", &hex::encode(&d[0..32])); keccak256_digest(d)})
            },
        }
    }
    fn code_size(&self, address: H160) -> usize {
        self.create_acc_if_not_exists(address);
        let accounts = self.accounts.borrow();
        match accounts.get(&address) {
            None => 0,
            Some(acc) => {
                acc.code(|d| d.len())
            },
        }
    }
    fn code(&self, address: H160) -> Vec<u8> {
        self.create_acc_if_not_exists(address);
        let accounts = self.accounts.borrow();
        match accounts.get(&address) {
            None => Vec::new(),
            Some(acc) => {
                acc.code(|d| d.into())
            },
        }
    }
    fn storage(&self, address: H160, index: H256) -> H256 {
        self.create_acc_if_not_exists(address);
        let accounts = self.accounts.borrow();
        match accounts.get(&address) {
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
            eprintln!("CreateScheme2 {} from {} {} {}", &hex::encode(_address), &hex::encode(caller) ,&hex::encode(code_hash), &hex::encode(salt));
        } else {
            eprintln!("Call create");
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
