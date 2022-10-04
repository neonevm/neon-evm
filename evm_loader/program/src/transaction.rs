use bytes::BytesMut;
use evm::{H160, U256};
use solana_program::{
    entrypoint::{ProgramResult},
    program_error::{ProgramError},
    secp256k1_recover::{secp256k1_recover},
};
use crate::account_storage::ProgramAccountStorage;
use crate::utils::{keccak256_digest};

#[derive(Debug)]
pub struct Transaction {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub to: Option<H160>,
    pub value: U256,
    pub call_data: Vec<u8>,
    pub v: U256,
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub chain_id: Option<U256>,
    pub recovery_id: u8,
    pub rlp_len: usize,
    pub hash: [u8; 32],
    pub signed_hash: [u8; 32],
}

impl Transaction {
    pub fn from_rlp(transaction: &[u8]) -> Result<Self, ProgramError> {
        rlp::decode(transaction)
            .map_err(|e| E!(ProgramError::InvalidInstructionData; "RLP DecoderError={}", e))
    }
}

impl rlp::Decodable for Transaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let info = rlp.payload_info()?;
        let payload_size = info.header_len + info.value_len;

        let nonce: u64 = rlp.val_at(0)?;
        let gas_price: U256 = rlp.val_at(1)?;
        let gas_limit: U256 = rlp.val_at(2)?;
        let to: Option<H160> = {
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
        };
        let value: U256 = rlp.val_at(4)?;
        let call_data: Vec<u8> = rlp.val_at(5)?;
        let v: U256 = rlp.val_at(6)?;

        let mut r: [u8; 32] = [0_u8; 32];
        let r_src: &[u8] = rlp.at(7)?.data()?;
        let r_pos: usize = r.len() - r_src.len();
        r[r_pos..].copy_from_slice(r_src);

        let mut s: [u8; 32] = [0_u8; 32];
        let s_src: &[u8] = rlp.at(8)?.data()?;
        let s_pos: usize = s.len() - s_src.len();
        s[s_pos..].copy_from_slice(s_src);

        let (chain_id, recovery_id) = if v >= U256::from(35) {
            let chain_id = (v - 1) / 2 - 17;
            let recovery_id = u8::from((v % 2).is_zero());
            (Some(chain_id), recovery_id)
        } else if v == U256::from(27) {
            (None, 0_u8)
        } else if v == U256::from(28) {
            (None, 1_u8)
        } else {
            return Err(rlp::DecoderError::RlpExpectedToBeData)
        };

        let raw = rlp.as_raw();
        let hash = solana_program::keccak::hash(&raw[..payload_size]).to_bytes();
        let signed_hash = signed_hash(rlp, chain_id)?;

        let tx = Self {
            nonce, gas_price, gas_limit, to, value, call_data, v, r, s,
            chain_id, recovery_id, rlp_len: payload_size, hash, signed_hash
        };

        Ok(tx)
    }
}

fn signed_hash(transaction: &rlp::Rlp, chain_id: Option<U256>) -> Result<[u8; 32], rlp::DecoderError> {
    let raw = transaction.as_raw();
    let payload_info = transaction.payload_info()?;
    let (_, v_offset) = transaction.at_with_offset(6)?;

    let list_len = if chain_id.is_some() { 9 } else { 6 };

    let buffer = BytesMut::with_capacity(raw.len());

    let mut rlp = rlp::RlpStream::new_list_with_buffer(buffer, list_len);
    rlp.append_raw(&raw[payload_info.header_len..v_offset], 6);

    if let Some(chain_id) = chain_id {
        rlp.append(&chain_id);
        rlp.append_empty_data();
        rlp.append_empty_data();
    }

    let hash = solana_program::keccak::hash(&rlp.out()).to_bytes();

    Ok(hash)
}

pub fn recover_caller_address(trx: &Transaction) -> Result<H160, ProgramError> {
    let signature = [trx.r, trx.s].concat();
    let public_key = secp256k1_recover(&trx.signed_hash, trx.recovery_id, &signature)
        .map_err(|e| E!(ProgramError::MissingRequiredSignature; "Secp256k1 Error={:?}", e))?;

    let address = keccak256_digest(&public_key.to_bytes());
    let address = H160::from_slice(&address[12..32]);

    Ok(address)
}

pub fn check_ethereum_transaction(
    account_storage: &ProgramAccountStorage,
    recovered_address: &H160,
    transaction: &Transaction
) -> ProgramResult {
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

    match &transaction.to {
        Some(address) => {
            if !transaction.call_data.is_empty() {
                let ether_account = account_storage.ethereum_account(address)
                    .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - target must be initialized account", address))?;

                if !ether_account.is_contract() {
                    return Err!(ProgramError::InvalidArgument; "Account {} - target must be a contract", address);
                }
            }
        }

        None => {
            let mut stream = rlp::RlpStream::new_list(2);
            stream.append(recovered_address);
            stream.append(&U256::from(transaction.nonce));
            let contract_address: H160 = crate::utils::keccak256_h256(&stream.out()).into();

            if let Some(account) = account_storage.ethereum_account(&contract_address) {
                if account.is_contract() {
                    return Err!(ProgramError::InvalidArgument; "Account {} is already a contract", contract_address)?;
                }
            }
        }
    }

    Ok(())
}
