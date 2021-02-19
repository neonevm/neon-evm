use bincode;
use impl_serde::serialize as bytes;
use rlp::RlpStream;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use solana_sdk::{
    entrypoint::ProgramResult, info, instruction::Instruction, program_error::ProgramError,
    secp256k1_program
};
use std::borrow::Cow;
use std::error::Error;
use std::convert::TryFrom;
pub use ethereum_types::{Address, U256};

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

pub fn make_secp256k1_instruction(instruction_index: u16, message_len: usize) -> Vec<u8> {
    let mut instruction_data = vec![];                    

    const CHECK_COUNT: u8 = 1;
    const DATA_START: u16 = 1;
    const ETH_SIZE: u16 = 20;
    const SIGN_SIZE: u16 = 65;
    const ETH_OFFSET: u16 = DATA_START;
    const SIGN_OFFSET: u16 = ETH_OFFSET + ETH_SIZE;
    const MSG_OFFSET: u16 = SIGN_OFFSET + SIGN_SIZE;

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

    instruction_data.push(1);
    instruction_data.extend(&bin_offsets);

    instruction_data
}


pub fn get_check_fields(raw_tx: &[u8]) {
    let data_start = 1 + 11;
    let eth_address_size = 20;
    let signature_size = 65;

    let (_, rest) = raw_tx.split_at(data_start);
    let (eth_adr, rest) = rest.split_at(eth_address_size);
    let (sign, msg) = rest.split_at(signature_size);

    info!(&("from: ".to_owned() + &hex::encode(&eth_adr)));
    info!(&("sign: ".to_owned() + &hex::encode(&sign)));
    info!(&(" msg: ".to_owned() + &hex::encode(&msg)));
}

pub fn check_tx(raw_tx: &[u8]) {
    let eth_tx: Result<SignedTransaction, _> = rlp::decode(&raw_tx);
    if eth_tx.is_err() {
        return;
    }

    let tx = eth_tx.unwrap();
    info!(&("         to: ".to_owned() + &tx.to.unwrap().to_string()));
    info!(&("      nonce: ".to_owned() + &tx.nonce.to_string()));
    info!(&("        gas: ".to_owned() + &tx.gas.to_string()));
    info!(&("  gas_price: ".to_owned() + &tx.gas_price.to_string()));
    info!(&("      value: ".to_owned() + &tx.value.to_string()));
    info!(&("       data: ".to_owned() + &hex::encode(&tx.data.0)));
    info!(&("          v: ".to_owned() + &tx.v.to_string()));
    info!(&("          r: ".to_owned() + &tx.r.to_string()));
    info!(&("          s: ".to_owned() + &tx.s.to_string()));
    info!("");

    let rlp_data = rlp::encode(&tx);
    info!(&("        msg: ".to_owned() + &hex::encode(&rlp_data)));    

    let mut r_bytes: [u8; 32] = [0; 32];
    let mut s_bytes: [u8; 32] = [0; 32];
    tx.r.to_big_endian(&mut r_bytes);
    let r = zpad(&r_bytes, 32);
    tx.s.to_big_endian(&mut s_bytes);
    let s = zpad(&s_bytes, 32);

    let mut compact_bytes: Vec<u8> = Vec::new();
    compact_bytes.extend(r);
    compact_bytes.extend(s);
    compact_bytes.push(u8::try_from(tx.v).unwrap());
    info!(&("       sign: ".to_owned() + &hex::encode(&compact_bytes))); 
}

pub fn get_data(raw_tx: &[u8]) -> (u64, Address, std::vec::Vec<u8>) {    
    let tx: Result<SignedTransaction, _> = rlp::decode(&raw_tx);
    let tx = tx.unwrap();

    (tx.nonce.as_u64(), tx.to.unwrap(), tx.data.0)
}

/// Hex-serialized shim for `Vec<u8>`.
#[derive(Serialize, Deserialize, Debug, Hash, PartialOrd, Ord, PartialEq, Eq, Clone, Default)]
pub struct Bytes(#[serde(with = "bytes")] pub Vec<u8>);
impl From<Vec<u8>> for Bytes {
    fn from(s: Vec<u8>) -> Self {
        Bytes(s)
    }
}

impl std::ops::Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0[..]
    }
}

#[derive(Clone)]
pub struct SignedTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub value: U256,
    pub data: Bytes,
    pub v: u64,
    pub r: U256,
    pub s: U256,
}

mod replay_protection {
    /// Adds chain id into v
    pub fn add(v: u8, chain_id: u64) -> u64 {
        v as u64 + 35 + chain_id * 2
    }

    /// Extracts chain_id from v
    pub fn chain_id(v: u64) -> Option<u64> {
        match v {
            v if v >= 35 => Some((v - 35) / 2),
            _ => None,
        }
    }
}

impl SignedTransaction {
    pub fn new(
        from: Address,
        to: Option<Address>,
        nonce: U256,
        gas: U256,
        gas_price: U256,
        value: U256,
        data: Bytes,
        chain_id: u64,
        v: u8,
        r: [u8; 32],
        s: [u8; 32],
    ) -> Self {
        let v = replay_protection::add(v, chain_id);
        let r = U256::from_big_endian(&r);
        let s = U256::from_big_endian(&s);

        Self {
            from,
            to,
            nonce,
            gas,
            gas_price,
            value,
            data,
            v,
            r,
            s,
        }
    }

    pub fn network_id(&self) -> Option<U256> {
        if self.r == U256::zero() && self.s == U256::zero() {
            Some(U256::from(self.v.clone()))
        } else if self.v == 27u32.into() || self.v == 28u32.into() {
            None
        } else {
            Some(((U256::from(self.v.clone()) - 1u32) / 2u32) - 17u32)
        }
    }
}

fn debug(s: &str, err: rlp::DecoderError) -> rlp::DecoderError {
    // log::error!("Error decoding field: {}: {:?}", s, err);
    err
}

impl rlp::Decodable for SignedTransaction {
    fn decode(d: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        if d.item_count()? != 9 {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        Ok(SignedTransaction {
            nonce: d.val_at(0).map_err(|e| debug("nonce", e))?,
            gas_price: d.val_at(1).map_err(|e| debug("gas_price", e))?,
            gas: d.val_at(2).map_err(|e| debug("gas", e))?,
            to: {
                let to = d.at(3).map_err(|e| debug("to", e))?;
                if to.is_empty() {
                    if to.is_data() {
                        None
                    } else {
                        return Err(rlp::DecoderError::RlpExpectedToBeData);
                    }
                } else {
                    Some(to.as_val().map_err(|e| debug("to", e))?)
                }
            },
            from: Default::default(),
            value: d.val_at(4).map_err(|e| debug("value", e))?,
            data: d.val_at::<Vec<u8>>(5).map_err(|e| debug("data", e))?.into(),
            v: d.val_at(6).map_err(|e| debug("v", e))?,
            r: d.val_at(7).map_err(|e| debug("r", e))?,
            s: d.val_at(8).map_err(|e| debug("s", e))?,
        })
    }
}

impl rlp::Encodable for SignedTransaction {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(9);
        s.append(&self.nonce);
        s.append(&self.gas_price);
        s.append(&self.gas);
        match self.to.as_ref() {
            None => s.append(&""),
            Some(addr) => s.append(addr),
        };
        s.append(&self.value);
        s.append(&self.data.0);

        let v = (self.v - 1) / 2 - 17;
        s.append(&v);
        s.append(&"");
        s.append(&"");
    }
}

//let data = vec![0x83, b'c', b'a', b't'];
//let decoded: SignedTransaction = rlp::decode(&data).unwrap();

/// Pad bytes with zeros at the beggining.
fn zpad(bytes: &[u8], len: usize) -> Vec<u8> {
    if bytes.len() >= len {
        return bytes.to_vec();
    }
    let mut pad = vec![0u8; len - bytes.len()];
    pad.extend(bytes);
    pad
}
