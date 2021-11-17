use solana_sdk::{
    clock::Slot,
    commitment_config::{CommitmentConfig, CommitmentLevel},
    instruction::{AccountMeta, Instruction},
    loader_instruction::LoaderInstruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer, Signature},
    signers::Signers,
    transaction::Transaction,
    system_program,
    system_instruction,
    sysvar::{clock},
    keccak::Hasher,
};

use serde::{Deserialize, Serialize};

use solana_clap_utils::{
    input_parsers::pubkey_of,
    input_validators::{is_url_or_moniker, is_valid_pubkey, normalize_to_url_if_moniker},
    keypair::{signer_from_path, keypair_from_path},
};

use std::{
    str::FromStr,
};


#[derive(Serialize, Deserialize)]
pub struct trx_t {
    pub from_addr: String,
    pub sign: String,
    pub msg: String,
    pub erc20_sol: String,
    pub erc20_eth: String,
    pub erc20_code: String,
    pub payer_sol: String,
    pub payer_eth: String,
    pub receiver_eth: String,
}

#[derive(Serialize, Deserialize)]
pub struct collateral_t{
    account : String,
    index: u32
}

pub fn make_keccak_instruction_data(instruction_index : u8, msg_len: u16, data_start : u16) ->Vec<u8> {
    let mut data = Vec::new();

    let check_count : u8 = 1;
    let eth_address_size : u16 = 20;
    let signature_size : u16 = 65;
    let eth_address_offset: u16 = data_start;
    let signature_offset : u16 = eth_address_offset + eth_address_size;
    let message_data_offset : u16 = signature_offset + signature_size;

    data.push(check_count);

    data.push(signature_offset as u8);
    data.push((signature_offset >> 8) as u8);

    data.push(instruction_index);

    data.push(eth_address_offset as u8);
    data.push((eth_address_offset >> 8) as u8);

    data.push(instruction_index);

    data.push(message_data_offset as u8);
    data.push((message_data_offset >> 8) as u8);

    data.push(msg_len as u8);
    data.push((msg_len >> 8) as u8);

    data.push(instruction_index);
    return data;
}


pub fn make_instruction_budget_units() -> Instruction{
    let DEFAULT_UNITS:u32 =500*1000;

    let instruction_unit = Instruction::new_with_bincode(
        Pubkey::from_str("ComputeBudget111111111111111111111111111111").unwrap(),
        &(0x00_u8, DEFAULT_UNITS),
        vec![]);

    instruction_unit
}

pub fn make_instruction_budget_heap() -> Instruction{
    let DEFAULT_HEAP_FRAME: u32=256*1024;

    let instruction_heap = Instruction::new_with_bincode(
        Pubkey::from_str("ComputeBudget111111111111111111111111111111").unwrap(),
        &(0x01_u8, DEFAULT_HEAP_FRAME),
        vec![]);

    instruction_heap
}

pub fn make_instruction_05(trx : &trx_t, evm_loader_key : &Pubkey, operator_sol : &Pubkey, collateral: &collateral_t) -> Instruction {

    let mut data_05_hex = String::from("05");
    data_05_hex.push_str(hex::encode(collateral.index.to_le_bytes()).as_str());
    data_05_hex.push_str(trx.from_addr.as_str());
    data_05_hex.push_str(trx.sign.as_str());
    data_05_hex.push_str(trx.msg.as_str());
    let data_05 : Vec<u8> = hex::decode(data_05_hex.as_str()).unwrap();

    let contract = Pubkey::from_str(trx.erc20_sol.as_str()).unwrap();
    let caller = Pubkey::from_str(trx.payer_sol.as_str()).unwrap();
    let sysinstruct = Pubkey::from_str("Sysvar1nstructions1111111111111111111111111").unwrap();
    let sysvarclock = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();
    let system = Pubkey::from_str("11111111111111111111111111111111").unwrap();
    let token_id = Pubkey::from_str("89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g").unwrap();
    let contract_token = spl_associated_token_account::get_associated_token_address(&contract, &token_id);
    let caller_token = spl_associated_token_account::get_associated_token_address(&caller, &token_id);
    let operator_token = spl_associated_token_account::get_associated_token_address(&operator_sol, &token_id);
    let collateral_pool_acc = Pubkey::from_str(collateral.account.as_str()).unwrap();

    let mut acc_meta = vec![

        AccountMeta::new_readonly(sysinstruct, false),
        AccountMeta::new(*operator_sol, true),
        AccountMeta::new(collateral_pool_acc, false),
        AccountMeta::new(operator_token, false),
        AccountMeta::new(caller_token, false),
        AccountMeta::new(system, false),

        AccountMeta::new(contract, false),
        AccountMeta::new(contract_token, false),
        // AccountMeta::new(contract_code, false),
        AccountMeta::new(caller, false),
        AccountMeta::new(caller_token, false),

        AccountMeta::new_readonly(*evm_loader_key, false),
        AccountMeta::new_readonly(token_id, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(sysvarclock, false),
    ];

    if (trx.erc20_code != ""){
        let contract_code = Pubkey::from_str(trx.erc20_code.as_str()).unwrap();
        acc_meta.insert(8, AccountMeta::new(contract_code, false));
    }

    let instruction_05 = Instruction::new_with_bytes(
        *evm_loader_key,
        &data_05,
        acc_meta);

    instruction_05
}
