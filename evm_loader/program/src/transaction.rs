use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use solana_program::{ 
    sysvar::instructions::{load_current_index, load_instruction_at},
    account_info::AccountInfo,
    entrypoint::{ ProgramResult },
    program_error::{ProgramError},
    secp256k1_program
};
use std::convert::Into;

#[derive(Default, Serialize, Deserialize, Debug)]
struct SecpSignatureOffsets {
    signature_offset: u16, // offset to [signature,recovery_id] of 64+1 bytes
    signature_instruction_index: u8,
    eth_address_offset: u16, // offset to eth_address of 20 bytes
    eth_address_instruction_index: u8,
    message_data_offset: u16, // offset to start of message data
    message_data_size: u16,   // size of message data
    message_instruction_index: u8,
}

pub fn make_secp256k1_instruction(instruction_index: u16, message_len: usize, data_start: u16) -> Vec<u8> {
    let mut instruction_data = vec![];

    const NUMBER_OF_SIGNATURES: u8 = 1;
    const ETH_SIZE: u16 = 20;
    const SIGN_SIZE: u16 = 65;
    let ETH_OFFSET: u16 = data_start;
    let SIGN_OFFSET: u16 = ETH_OFFSET + ETH_SIZE;
    let MSG_OFFSET: u16 = SIGN_OFFSET + SIGN_SIZE;

    let offsets = SecpSignatureOffsets {
        signature_offset: SIGN_OFFSET as u16,
        signature_instruction_index: instruction_index as u8,
        eth_address_offset: ETH_OFFSET as u16,
        eth_address_instruction_index: instruction_index as u8,
        message_data_offset: MSG_OFFSET as u16,
        message_data_size: message_len as u16,
        message_instruction_index: instruction_index as u8,
    };

    let bin_offsets = bincode::serialize(&offsets).unwrap();

    instruction_data.push(NUMBER_OF_SIGNATURES);
    instruction_data.extend(&bin_offsets);

    instruction_data
}

pub fn check_secp256k1_instruction(sysvar_info: &AccountInfo, message_len: usize, data_offset: u16) -> ProgramResult
{
    let current_instruction = load_current_index(&sysvar_info.try_borrow_data()?);
    let index = current_instruction - 1;

    match load_instruction_at(index.into(), &sysvar_info.try_borrow_data()?) {
        Ok(instr) => {
            if secp256k1_program::check_id(&instr.program_id) {
                let reference_instruction = make_secp256k1_instruction(current_instruction, message_len, data_offset);
                if reference_instruction != instr.data {
                    debug_print!("wrong keccak instruction data");
                    debug_print!("instruction: {}", &hex::encode(&instr.data));
                    debug_print!("reference: {}", &hex::encode(&reference_instruction));
                    return Err(ProgramError::InvalidInstructionData);
                }
            } else {
                return Err(ProgramError::IncorrectProgramId);
            }
        },
        Err(err) => {
            debug_print!("ERR");
            return Err(ProgramError::MissingRequiredSignature);
        }
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
    pub chain_id: U256,
}

impl rlp::Decodable for UnsignedTransaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if rlp.item_count()? != 9 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let tx = UnsignedTransaction {
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
            chain_id: rlp.val_at(6)?,
        };

        Ok(tx)
    }
}

pub fn get_data(raw_tx: &[u8]) -> (u64, Option<H160>, Vec<u8>) {
    let tx: Result<UnsignedTransaction, _> = rlp::decode(&raw_tx);
    let tx = tx.unwrap();

    (tx.nonce, tx.to, tx.call_data)
}

pub fn verify_tx_signature(signature: &[u8], unsigned_trx: &[u8]) -> Result<(), secp256k1::Error> {
    let digest = Keccak256::digest(unsigned_trx);
    let message = secp256k1::Message::parse_slice(&digest)?;

    let recovery_id = secp256k1::RecoveryId::parse(signature[64])?;
    let signature = secp256k1::Signature::parse_slice(&signature[0..64])?;

    let public_key = secp256k1::recover(&message, &signature, &recovery_id)?;
    if secp256k1::verify(&message, &signature, &public_key) {
        Ok(())
    } else {
        Err(secp256k1::Error::InvalidSignature)
    }
}
