use rlp::RlpStream;
use solana_program::{
    keccak::{hash,},
};
use solana_sdk::{
    clock::Slot,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    pubkey::Pubkey,
};
use std::{
    rc::Rc,
    sync::Arc,
};

use libsecp256k1::{SecretKey, Signature};
use libsecp256k1::PublicKey;
use evm::{H160, H256, U256};

use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcSendTransactionConfig, RpcTransactionConfig},
    rpc_request::MAX_GET_SIGNATURE_STATUSES_QUERY_ITEMS,
    tpu_client::{TpuClient, TpuClientConfig},
};

use evm_loader::{
    instruction::EvmInstruction,
    // solana_backend::SolanaBackend,
    account_data::{AccountData, Account, Contract},
};

const CHAIN_ID :u32 = 245022940;

type Error = Box<dyn std::error::Error>;


#[derive(Debug)]
pub struct UnsignedTransaction {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<H160>,
    pub value: U256,
    pub data: Vec<u8>,
    pub chain_id: U256,
}

impl rlp::Encodable for UnsignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas_limit);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(addr) => s.append(addr),
        };
        s.append(&self.value);
        s.append(&self.data);
        s.append(&self.chain_id);
        s.append_empty_data();
        s.append_empty_data();
    }
}


#[must_use]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    hash(data).to_bytes()
}


pub fn make_ethereum_transaction(
    rpc_client: &Arc<RpcClient>,
    caller: &Pubkey,
    to: H160,
    caller_private_bin: &[u8; 32]
) -> (Vec<u8>, Vec<u8>) {

    let caller_private = SecretKey::parse(&caller_private_bin).unwrap();
    let trx_count = get_ether_nonce(rpc_client, caller).unwrap();

    let rlp_data = {
        let tx = UnsignedTransaction {
            to: Some(to),
            nonce: trx_count,
            gas_limit: 9_999_999.into(),
            gas_price: 0.into(),
            value: 10_u64.pow(9).into(),
            data: vec![],
            chain_id: CHAIN_ID.into(),
        };

        rlp::encode(&tx).to_vec()
    };

    let (r_s, v) = {
        use libsecp256k1::{Message, sign};
        let msg = Message::parse(&keccak256(rlp_data.as_slice()));
        sign(&msg, &caller_private)
    };

    let mut signature : Vec<u8> = Vec::new();
    signature.extend(r_s.serialize().iter().copied());
    signature.push(v.serialize());

    (signature, rlp_data)
}

fn get_ether_nonce(
    rpc_client: &Arc<RpcClient>,
    caller_sol: &Pubkey
) -> Result<(u64), Error> {

    let data : Vec<u8>;
    match rpc_client.get_account_with_commitment(caller_sol, CommitmentConfig::confirmed())?.value{
        Some(acc) =>   data = acc.data,
        None => return Ok(u64::default())
    }

    let trx_count : u64;
    println!("get_ether_account_nonce data = {:?}", data);
    let account = match evm_loader::account_data::AccountData::unpack(&data) {
        Ok(acc_data) =>
            match acc_data {
                AccountData::Account(acc) => acc,
                _ => return Err("Caller has incorrect type".into())
            },
        Err(_) => return Err("Caller unpack error".into())
    };
    trx_count = account.trx_count;
    // let caller_ether = account.ether;
    // let caller_token = spl_associated_token_account::get_associated_token_address(caller_sol, &token_mint::id());

    // println!("Caller: ether {}, solana {}", caller_ether, caller_sol);
    println!("Caller trx_count: {} ", trx_count);
    // println!("caller_token = {}", caller_token);

    Ok(trx_count)
}
