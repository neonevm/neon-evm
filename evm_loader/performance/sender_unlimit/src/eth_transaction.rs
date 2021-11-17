use rlp::RlpStream;
use solana_program::{
    keccak::{hash,},
};


use libsecp256k1::SecretKey;
use libsecp256k1::PublicKey;
use evm::{H160, H256, U256};


const CHAIN_ID :u32 = 245022940;



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


fn make_ethereum_transaction(
    to: H160,
    trx_count: u64,
    value: u32,
    program_data: &[u8],
    caller_private: &SecretKey
) -> Vec<u8> {

    let rlp_data = {
        let tx = UnsignedTransaction {
            to: Some(to),
            nonce: trx_count,
            gas_limit: 9_999_999.into(),
            gas_price: 0.into(),
            value: value.into(),
            data: program_data.to_owned(),
            chain_id: CHAIN_ID.into(),
        };

        rlp::encode(&tx).to_vec()
    };

    let (sig, rec) = {
        use libsecp256k1::{Message, sign};
        let msg = Message::parse(&keccak256(rlp_data.as_slice()));
        sign(&msg, caller_private)
    };

    let mut msg : Vec<u8> = Vec::new();
    msg.extend(sig.serialize().iter().copied());
    msg.push(rec.serialize());
    msg.extend((rlp_data.len() as u64).to_le_bytes().iter().copied());
    msg.extend(rlp_data);

    msg
}
