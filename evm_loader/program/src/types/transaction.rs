use ethnum::U256;
use std::convert::{TryInto};
use crate::error::Error;

use super::Address;

#[derive(Default)]
pub struct Transaction {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: U256,
    pub target: Option<Address>,
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
    pub fn from_rlp(transaction: &[u8]) -> Result<Self, Error> {
        rlp::decode(transaction).map_err(Error::from)
    }

    pub fn recover_caller_address(&self) -> Result<Address, Error> {
        use solana_program::keccak::{hash, Hash};
        use solana_program::secp256k1_recover::secp256k1_recover;

        let signature = [self.r, self.s].concat();
        let public_key = secp256k1_recover(&self.signed_hash, self.recovery_id, &signature)?;
    
        let Hash(address) = hash(&public_key.to_bytes());
        let address: [u8; 20] = address[12..32].try_into()?;
    
        Ok(Address::from(address))
    }
}

impl rlp::Decodable for Transaction {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        #[inline]
        fn u256(rlp: &rlp::Rlp) -> Result<U256, rlp::DecoderError> {
            rlp.decoder().decode_value(|bytes| {
                if !bytes.is_empty() && bytes[0] == 0 {
                    Err(rlp::DecoderError::RlpInvalidIndirection)
                } else if bytes.len() <= 32 {
                    let mut buffer = [0_u8; 32];
                    buffer[(32 - bytes.len())..].copy_from_slice(bytes);
                    Ok(U256::from_be_bytes(buffer))
                } else {
                    Err(rlp::DecoderError::RlpIsTooBig)
                }
            })
        }

        let info = rlp.payload_info()?;
        let payload_size = info.header_len + info.value_len;

        let nonce: u64 = rlp.val_at(0)?;
        let gas_price: U256 = u256(&rlp.at(1)?)?;
        let gas_limit: U256 = u256(&rlp.at(2)?)?;
        let target: Option<Address> = {
            let target = rlp.at(3)?;
            if target.is_empty() {
                if target.is_data() {
                    None
                } else {
                    return Err(rlp::DecoderError::RlpExpectedToBeData);
                }
            } else {
                Some(target.as_val()?)
            }
        };
        let value: U256 = u256(&rlp.at(4)?)?;
        let call_data: Vec<u8> = rlp.val_at(5)?;
        let v: U256 = u256(&rlp.at(6)?)?;

        let mut r: [u8; 32] = [0_u8; 32];
        let r_src: &[u8] = rlp.at(7)?.data()?;
        let r_pos: usize = r.len() - r_src.len();
        r[r_pos..].copy_from_slice(r_src);

        let mut s: [u8; 32] = [0_u8; 32];
        let s_src: &[u8] = rlp.at(8)?.data()?;
        let s_pos: usize = s.len() - s_src.len();
        s[s_pos..].copy_from_slice(s_src);

        let (chain_id, recovery_id) = if v >= 35 {
            let chain_id = (v - 1) / 2 - 17;
            let recovery_id = u8::from((v % 2) == U256::ZERO);
            (Some(chain_id), recovery_id)
        } else if v == 27 {
            (None, 0_u8)
        } else if v == 28 {
            (None, 1_u8)
        } else {
            return Err(rlp::DecoderError::RlpExpectedToBeData)
        };

        let raw = rlp.as_raw();
        let hash = solana_program::keccak::hash(&raw[..payload_size]).to_bytes();
        let signed_hash = signed_hash(rlp, chain_id)?;

        let tx = Self {
            nonce, gas_price, gas_limit, target, value, call_data, v, r, s,
            chain_id, recovery_id, rlp_len: payload_size, hash, signed_hash
        };

        Ok(tx)
    }
}

fn signed_hash(transaction: &rlp::Rlp, chain_id: Option<U256>) -> Result<[u8; 32], rlp::DecoderError> {
    let raw = transaction.as_raw();
    let payload_info = transaction.payload_info()?;
    let (_, v_offset) = transaction.at_with_offset(6)?;

    let middle = &raw[payload_info.header_len..v_offset];

    let trailer = chain_id.map_or_else(
        Vec::new,
        |chain_id| {
            let chain_id = {
                let leading_empty_bytes = (chain_id.leading_zeros() as usize) / 8;
                let bytes = chain_id.to_be_bytes();
                bytes[leading_empty_bytes..].to_vec()
            };

            let mut trailer = Vec::with_capacity(64);
            match chain_id.len() {
                0 => {
                    trailer.extend_from_slice(&[0x80]);
                },
                1 if chain_id[0] < 0x80 => {
                    trailer.extend_from_slice(&chain_id);
                },
                len @ 1..=55 => {
                    let len: u8 = len.try_into().unwrap();

                    trailer.extend_from_slice(&[0x80 + len]);
                    trailer.extend_from_slice(&chain_id);
                },
                _ => {
                    unreachable!("chain_id.len() <= 32")
                }
            }

            trailer.extend_from_slice(&[0x80, 0x80]);
            trailer
        }
    );

    let header: Vec<u8> = {
        let len = middle.len() + trailer.len();
        if len <= 55 {
            let len: u8 = len.try_into().unwrap();
            vec![0xC0 + len]
        } else {
            let len_bytes = {
                let leading_empty_bytes = (len.leading_zeros() as usize) / 8;
                let bytes = len.to_be_bytes();
                bytes[leading_empty_bytes..].to_vec()
            };
            let len_bytes_len: u8 = len_bytes.len().try_into().unwrap();

            let mut header = Vec::with_capacity(10);
            header.extend_from_slice(&[0xF7 + len_bytes_len]);
            header.extend_from_slice(&len_bytes);

            header
        }
    };

    let hash = solana_program::keccak::hashv(
        &[&header, middle, &trailer]
    ).to_bytes();

    Ok(hash)
}
