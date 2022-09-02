use evm::{H160, U256};
use solana_program::{ 
    sysvar::instructions::{load_current_index_checked, load_instruction_at_checked},
    account_info::AccountInfo,
    entrypoint::{ ProgramResult },
    program_error::{ProgramError},
    secp256k1_program,
    secp256k1_recover::{secp256k1_recover},
};
use std::convert::{Into, TryFrom};
use crate::account_storage::ProgramAccountStorage;
use crate::utils::{keccak256_digest};

#[repr(packed)]
#[allow(dead_code)]
struct SecpSignatureOffsets {
    signature_offset: u16, // offset to [signature,recovery_id] of 64+1 bytes
    signature_instruction_index: u8,
    eth_address_offset: u16, // offset to eth_address of 20 bytes
    eth_address_instruction_index: u8,
    message_data_offset: u16, // offset to start of message data
    message_data_size: u16,   // size of message data
    message_instruction_index: u8,
}

#[must_use]
pub fn make_secp256k1_instruction(instruction_index: u8, message_len: u16, data_start: u16) -> Vec<u8> {
    const NUMBER_OF_SIGNATURES: u8 = 1;
    const ETH_SIZE: u16 = 20;
    const SIGN_SIZE: u16 = 65;
    let eth_offset: u16 = data_start;
    let sign_offset: u16 = eth_offset + ETH_SIZE;
    let msg_offset: u16 = sign_offset + SIGN_SIZE;

    let offsets = SecpSignatureOffsets {
        signature_offset: sign_offset,
        signature_instruction_index: instruction_index,
        eth_address_offset: eth_offset,
        eth_address_instruction_index: instruction_index,
        message_data_offset: msg_offset,
        message_data_size: message_len,
        message_instruction_index: instruction_index,
    };

    let bin_offsets: [u8; 11] = unsafe { core::mem::transmute(offsets) };

    let mut instruction_data = Vec::with_capacity(1 + bin_offsets.len());
    instruction_data.push(NUMBER_OF_SIGNATURES);
    instruction_data.extend(bin_offsets);

    instruction_data
}

pub fn check_secp256k1_instruction(sysvar_info: &AccountInfo, message_len: usize, data_offset: u16) -> ProgramResult
{
    if !solana_program::sysvar::instructions::check_id(sysvar_info.key) {
        return Err!(ProgramError::InvalidAccountData; "Invalid sysvar instruction account {}", sysvar_info.key);
    }

    let message_len = u16::try_from(message_len).map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
    
    let current_instruction = load_current_index_checked(sysvar_info)?;
    let current_instruction = u8::try_from(current_instruction).map_err(|e| E!(ProgramError::InvalidInstructionData; "TryFromIntError={:?}", e))?;
    let index = current_instruction - 1;

    if let Ok(instr) = load_instruction_at_checked(index.into(), sysvar_info) {
        if secp256k1_program::check_id(&instr.program_id) {
            let reference_instruction = make_secp256k1_instruction(current_instruction, message_len, data_offset);
            if reference_instruction != instr.data {
                return Err!(ProgramError::InvalidInstructionData; "wrong keccak instruction data, instruction={}, reference={}", &hex::encode(&instr.data), &hex::encode(&reference_instruction));
            }
        } else {
            return Err!(ProgramError::IncorrectProgramId; "Incorrect Program Id: index={:?}, sysvar_info={:?}, instr.program_id={:?}", index, sysvar_info, instr.program_id);
        }
    }
    else {
        return Err!(ProgramError::MissingRequiredSignature; "index={:?}, sysvar_info={:?}", index, sysvar_info);
    }

    Ok(())
}


#[derive(Debug)]
pub struct UnsignedTransaction {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<H160>,
    pub value: U256,
    pub call_data: Vec<u8>,
    pub chain_id: Option<U256>,
    pub rlp_len: usize,
}

impl UnsignedTransaction {
    pub fn from_rlp(unsigned_msg: &[u8]) -> Result<Self, ProgramError> {
        let trx = rlp::decode(unsigned_msg)
            .map_err(|e| E!(ProgramError::InvalidInstructionData; "RLP DecoderError={}", e))?;

        Ok(trx)
    }
}

impl rlp::Decodable for UnsignedTransaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let field_count = rlp.item_count()?;
        match field_count {
            6 | 9 => (),
            _ => return Err(rlp::DecoderError::RlpIncorrectListLen),
        }

        let info = rlp.payload_info()?;
        let payload_size = info.header_len + info.value_len;

        let tx = Self {
            nonce: rlp.val_at(0)?,
            gas_price: rlp.val_at(1)?,
            gas_limit: rlp.val_at(2)?,
            to: {
                let to = rlp.at(3)?;
                if to.is_empty() {
                    if to.is_data() {
                        None
                    } else {
                        return Err(rlp::DecoderError::RlpExpectedToBeData);
                    }
                } else {
                    Some(to.as_val()?)
                }
            },
            value: rlp.val_at(4)?,
            call_data: rlp.val_at(5)?,
            chain_id: if field_count == 6 {
                None
            } else {
                // Although v size is not limited by the specification, we don't expect it
                // to be higher, so make the code simpler:
                Some(rlp.val_at(6)?)
            },
            rlp_len: payload_size,
        };

        Ok(tx)
    }
}


pub fn verify_tx_signature(signature: &[u8; 65], unsigned_trx: &[u8]) -> Result<H160, ProgramError> {
    let digest = keccak256_digest(unsigned_trx);

    let public_key = secp256k1_recover(&digest, signature[64], &signature[0..64])
        .map_err(|e| E!(ProgramError::MissingRequiredSignature; "Secp256k1 Error={:?}", e))?;

    let address = keccak256_digest(&public_key.to_bytes());
    let address = H160::from_slice(&address[12..32]);

    Ok(address)
}

pub fn check_ethereum_transaction(
    account_storage: &ProgramAccountStorage,
    recovered_address: &H160,
    transaction: &UnsignedTransaction
) -> ProgramResult
{
    let sender_account = account_storage.ethereum_account(recovered_address)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - sender must be initialized account", recovered_address))?;

    if sender_account.trx_count != transaction.nonce {
        return Err!(ProgramError::InvalidArgument; "Invalid Ethereum transaction nonce: acc {}, trx {}", sender_account.trx_count, transaction.nonce);
    }

    if let Some(ref chain_id) = transaction.chain_id {
        if &U256::from(crate::config::CHAIN_ID) != chain_id {
            return Err!(ProgramError::InvalidArgument; "Invalid chain_id: actual {}, expected {}", chain_id, crate::config::CHAIN_ID);
        }
    }

    let contract_address: H160 = transaction.to.unwrap_or_else(|| {
        let mut stream = rlp::RlpStream::new_list(2);
        stream.append(recovered_address);
        stream.append(&U256::from(transaction.nonce));
        crate::utils::keccak256_h256(&stream.out()).into()
    });
    let contract_account = account_storage.ethereum_account(&contract_address)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - target must be initialized account", contract_address))?;

    if !transaction.call_data.is_empty() && contract_account.code_account.is_none() {
        return Err!(ProgramError::InvalidArgument; "Account {} - target must be contract account", contract_address);
    }

    Ok(())
}
