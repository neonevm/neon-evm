use ethnum::U256;
use maybe_async::maybe_async;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;

use crate::{
    account_storage::AccountStorage, config::GAS_LIMIT_MULTIPLIER_NO_CHAINID, error::Error,
};

use super::{
    serde::{bytes_32, option_u256},
    Address,
};

#[repr(transparent)]
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct StorageKey(#[serde(with = "bytes_32")] [u8; 32]);

impl rlp::Decodable for StorageKey {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        rlp.decoder().decode_value(|bytes| {
            let array: [u8; 32] = bytes
                .try_into()
                .map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
            Ok(Self(array))
        })
    }
}

impl TryFrom<Vec<u8>> for StorageKey {
    type Error = String;

    fn try_from(hex: Vec<u8>) -> Result<Self, Self::Error> {
        let bytes = hex;

        if bytes.len() != 32 {
            return Err(String::from("Hex string must be 32 bytes"));
        }

        let mut array = [0; 32];
        array.copy_from_slice(&bytes);

        Ok(StorageKey(array))
    }
}

impl AsRef<[u8]> for StorageKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub enum TransactionEnvelope {
    Legacy,
    AccessList,
    DynamicFee,
}

impl TransactionEnvelope {
    pub fn get_type(bytes: &[u8]) -> (Option<TransactionEnvelope>, &[u8]) {
        // Legacy transaction format
        if rlp::Rlp::new(bytes).is_list() {
            (None, bytes)
        // It's an EIP-2718 typed TX envelope.
        } else {
            match bytes[0] {
                0x00 => (Some(TransactionEnvelope::Legacy), &bytes[1..]),
                0x01 => (Some(TransactionEnvelope::AccessList), &bytes[1..]),
                0x02 => (Some(TransactionEnvelope::DynamicFee), &bytes[1..]),
                byte => panic!("Unsupported EIP-2718 Transaction type | First byte: {byte}"),
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LegacyTx {
    pub nonce: u64,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub gas_price: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub gas_limit: U256,
    pub target: Option<Address>,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub call_data: Vec<u8>,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub v: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub r: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub s: U256,
    #[serde(with = "option_u256")]
    pub chain_id: Option<U256>,
    pub recovery_id: u8,
}

impl rlp::Decodable for LegacyTx {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let rlp_len = {
            let info = rlp.payload_info()?;
            info.header_len + info.value_len
        };

        if rlp.as_raw().len() != rlp_len {
            return Err(rlp::DecoderError::RlpInconsistentLengthAndData);
        }

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
        let call_data = rlp.val_at(5)?;
        let v: U256 = u256(&rlp.at(6)?)?;
        let r: U256 = u256(&rlp.at(7)?)?;
        let s: U256 = u256(&rlp.at(8)?)?;

        if rlp.at(9).is_ok() {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let (chain_id, recovery_id) = if v >= 35 {
            let chain_id = (v - 1) / 2 - 17;
            let recovery_id = u8::from((v % 2) == U256::ZERO);
            (Some(chain_id), recovery_id)
        } else if v == 27 {
            (None, 0_u8)
        } else if v == 28 {
            (None, 1_u8)
        } else {
            return Err(rlp::DecoderError::RlpExpectedToBeData);
        };

        let tx = LegacyTx {
            nonce,
            gas_price,
            gas_limit,
            target,
            value,
            call_data,
            v,
            r,
            s,
            chain_id,
            recovery_id,
        };

        Ok(tx)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessListTx {
    pub nonce: u64,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub gas_price: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub gas_limit: U256,
    pub target: Option<Address>,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub value: U256,
    #[serde(with = "serde_bytes")]
    pub call_data: Vec<u8>,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub r: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub s: U256,
    #[serde(with = "ethnum::serde::bytes::le")]
    pub chain_id: U256,
    pub recovery_id: u8,
    pub access_list: Vec<(Address, Vec<StorageKey>)>,
}

impl rlp::Decodable for AccessListTx {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        let rlp_len = {
            let info = rlp.payload_info()?;
            info.header_len + info.value_len
        };

        if rlp.as_raw().len() != rlp_len {
            return Err(rlp::DecoderError::RlpInconsistentLengthAndData);
        }

        let chain_id: U256 = u256(&rlp.at(0)?)?;
        let nonce: u64 = rlp.val_at(1)?;
        let gas_price: U256 = u256(&rlp.at(2)?)?;
        let gas_limit: U256 = u256(&rlp.at(3)?)?;
        let target: Option<Address> = {
            let target = rlp.at(4)?;
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

        let value: U256 = u256(&rlp.at(5)?)?;
        let call_data = rlp.val_at(6)?;

        let rlp_access_list = rlp.at(7)?;
        let mut access_list = vec![];

        for entry in &rlp_access_list {
            // Check if entry is a list
            if entry.is_list() {
                // Parse address from first element
                let address: Address = entry.at(0)?.as_val()?;

                // Get storage keys from second element
                let mut storage_keys: Vec<StorageKey> = vec![];

                for key in &entry.at(1)? {
                    storage_keys.push(key.as_val()?);
                }

                access_list.push((address, storage_keys));
            } else {
                return Err(rlp::DecoderError::RlpExpectedToBeList);
            }
        }

        let y_parity: u8 = rlp.at(8)?.as_val()?;
        let r: U256 = u256(&rlp.at(9)?)?;
        let s: U256 = u256(&rlp.at(10)?)?;

        if rlp.at(11).is_ok() {
            return Err(rlp::DecoderError::RlpIncorrectListLen);
        }

        let tx = AccessListTx {
            nonce,
            gas_price,
            gas_limit,
            target,
            value,
            call_data,
            r,
            s,
            chain_id,
            recovery_id: y_parity,
            access_list,
        };

        Ok(tx)
    }
}

// TODO: Will be added as a part of EIP-1559
// struct DynamicFeeTx {}

#[derive(Debug, Serialize, Deserialize)]
pub enum TransactionPayload {
    Legacy(LegacyTx),
    AccessList(AccessListTx),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub transaction: TransactionPayload,
    pub byte_len: usize,
    #[serde(with = "bytes_32")]
    pub hash: [u8; 32],
    #[serde(with = "bytes_32")]
    pub signed_hash: [u8; 32],
}

impl Transaction {
    pub fn from_payload(
        transaction_type: &Option<TransactionEnvelope>,
        chain_id: Option<U256>,
        transaction_rlp: &rlp::Rlp,
        transaction: TransactionPayload,
    ) -> Result<Self, rlp::DecoderError> {
        let (hash, signed_hash) = match *transaction_type {
            // Legacy transaction wrapped in envelop
            Some(TransactionEnvelope::Legacy) => {
                let hash =
                    solana_program::keccak::hashv(&[&[0x00], transaction_rlp.as_raw()]).to_bytes();
                let signed_hash = Self::calculate_legacy_signature(transaction_rlp, chain_id)?;

                (hash, signed_hash)
            }
            // Access List transaction
            Some(TransactionEnvelope::AccessList) => {
                let hash =
                    solana_program::keccak::hashv(&[&[0x01], transaction_rlp.as_raw()]).to_bytes();
                let signed_hash = Self::eip2718_signed_hash(&[0x01], transaction_rlp, 8)?;

                (hash, signed_hash)
            }
            // Legacy trasaction
            None => {
                let hash = solana_program::keccak::hash(transaction_rlp.as_raw()).to_bytes();
                let signed_hash = Self::calculate_legacy_signature(transaction_rlp, chain_id)?;

                (hash, signed_hash)
            }
            _ => unimplemented!(),
        };

        let info = transaction_rlp.payload_info()?;
        let byte_len = if transaction_type.is_none() {
            // Legacy transaction
            info.header_len + info.value_len
        } else {
            // Transaction in the type envelope
            info.header_len + info.value_len + 1 // + 1 byte for type
        };

        Ok(Transaction {
            transaction,
            byte_len,
            hash,
            signed_hash,
        })
    }

    fn eip2718_signed_hash(
        transaction_type: &[u8],
        transaction: &rlp::Rlp,
        middle_offset: usize,
    ) -> Result<[u8; 32], rlp::DecoderError> {
        let raw = transaction.as_raw();
        let payload_info = transaction.payload_info()?;
        let (_, middle_offset) = transaction.at_with_offset(middle_offset)?;

        let body = &raw[payload_info.header_len..middle_offset];

        let header: Vec<u8> = {
            let len = body.len();
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

        let hash = solana_program::keccak::hashv(&[transaction_type, &header, body]).to_bytes();

        Ok(hash)
    }

    fn calculate_legacy_signature(
        transaction: &rlp::Rlp,
        chain_id: Option<U256>,
    ) -> Result<[u8; 32], rlp::DecoderError> {
        let raw = transaction.as_raw();
        let payload_info = transaction.payload_info()?;
        let (_, v_offset) = transaction.at_with_offset(6)?;

        let middle = &raw[payload_info.header_len..v_offset];

        let trailer = chain_id.map_or_else(Vec::new, |chain_id| {
            let chain_id = {
                let leading_empty_bytes = (chain_id.leading_zeros() as usize) / 8;
                let bytes = chain_id.to_be_bytes();
                bytes[leading_empty_bytes..].to_vec()
            };

            let mut trailer = Vec::with_capacity(64);
            match chain_id.len() {
                0 => {
                    trailer.extend_from_slice(&[0x80]);
                }
                1 if chain_id[0] < 0x80 => {
                    trailer.extend_from_slice(&chain_id);
                }
                len @ 1..=55 => {
                    let len: u8 = len.try_into().unwrap();

                    trailer.extend_from_slice(&[0x80 + len]);
                    trailer.extend_from_slice(&chain_id);
                }
                _ => {
                    unreachable!("chain_id.len() <= 32")
                }
            }

            trailer.extend_from_slice(&[0x80, 0x80]);
            trailer
        });

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

        let hash = solana_program::keccak::hashv(&[&header, middle, &trailer]).to_bytes();

        Ok(hash)
    }
}

impl Transaction {
    pub fn from_rlp(transaction: &[u8]) -> Result<Self, Error> {
        let (transaction_type, transaction) = TransactionEnvelope::get_type(transaction);

        let tx = match transaction_type {
            Some(TransactionEnvelope::Legacy) => {
                let legacy_tx = rlp::decode::<LegacyTx>(transaction).map_err(Error::from)?;
                let chain_id = legacy_tx.chain_id;
                let tx = TransactionPayload::Legacy(legacy_tx);
                Transaction::from_payload(
                    &Some(TransactionEnvelope::Legacy),
                    chain_id,
                    &rlp::Rlp::new(transaction),
                    tx,
                )?
            }
            Some(TransactionEnvelope::AccessList) => {
                let access_list_tx =
                    rlp::decode::<AccessListTx>(transaction).map_err(Error::from)?;
                let chain_id = access_list_tx.chain_id;
                let tx = TransactionPayload::AccessList(access_list_tx);
                Transaction::from_payload(
                    &Some(TransactionEnvelope::AccessList),
                    Some(chain_id),
                    &rlp::Rlp::new(transaction),
                    tx,
                )?
            }
            None => {
                let legacy_tx = rlp::decode::<LegacyTx>(transaction).map_err(Error::from)?;
                let chain_id = legacy_tx.chain_id;
                let tx = TransactionPayload::Legacy(legacy_tx);
                Transaction::from_payload(&None, chain_id, &rlp::Rlp::new(transaction), tx)?
            }
            Some(TransactionEnvelope::DynamicFee) => unimplemented!(),
        };

        Ok(tx)
    }

    pub fn recover_caller_address(&self) -> Result<Address, Error> {
        use solana_program::keccak::{hash, Hash};
        use solana_program::secp256k1_recover::secp256k1_recover;

        let signature = [self.r().to_be_bytes(), self.s().to_be_bytes()].concat();
        let public_key = secp256k1_recover(&self.signed_hash(), self.recovery_id(), &signature)?;

        let Hash(address) = hash(&public_key.to_bytes());
        let address: [u8; 20] = address[12..32].try_into()?;

        Ok(Address::from(address))
    }

    #[must_use]
    pub fn nonce(&self) -> u64 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { nonce, .. })
            | TransactionPayload::AccessList(AccessListTx { nonce, .. }) => nonce,
        }
    }

    #[must_use]
    pub fn gas_price(&self) -> U256 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { gas_price, .. })
            | TransactionPayload::AccessList(AccessListTx { gas_price, .. }) => gas_price,
        }
    }

    #[must_use]
    pub fn gas_limit(&self) -> U256 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { gas_limit, .. })
            | TransactionPayload::AccessList(AccessListTx { gas_limit, .. }) => gas_limit,
        }
    }

    pub fn gas_limit_in_tokens(&self) -> Result<U256, Error> {
        self.gas_price()
            .checked_mul(self.gas_limit())
            .ok_or(Error::IntegerOverflow)
    }

    #[must_use]
    pub fn target(&self) -> Option<Address> {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { target, .. })
            | TransactionPayload::AccessList(AccessListTx { target, .. }) => target,
        }
    }

    #[must_use]
    pub fn value(&self) -> U256 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { value, .. })
            | TransactionPayload::AccessList(AccessListTx { value, .. }) => value,
        }
    }

    #[must_use]
    pub fn call_data(&self) -> &[u8] {
        match &self.transaction {
            TransactionPayload::Legacy(LegacyTx { call_data, .. })
            | TransactionPayload::AccessList(AccessListTx { call_data, .. }) => call_data,
        }
    }

    #[must_use]
    pub fn r(&self) -> U256 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { r, .. })
            | TransactionPayload::AccessList(AccessListTx { r, .. }) => r,
        }
    }

    #[must_use]
    pub fn s(&self) -> U256 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { s, .. })
            | TransactionPayload::AccessList(AccessListTx { s, .. }) => s,
        }
    }

    #[must_use]
    pub fn chain_id(&self) -> Option<u64> {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { chain_id, .. }) => chain_id,
            TransactionPayload::AccessList(AccessListTx { chain_id, .. }) => Some(chain_id),
        }
        .map(std::convert::TryInto::try_into)
        .transpose()
        .expect("chain_id < u64::max")
    }

    #[must_use]
    pub fn recovery_id(&self) -> u8 {
        match self.transaction {
            TransactionPayload::Legacy(LegacyTx { recovery_id, .. })
            | TransactionPayload::AccessList(AccessListTx { recovery_id, .. }) => recovery_id,
        }
    }

    #[must_use]
    pub fn rlp_len(&self) -> usize {
        self.byte_len
    }

    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    #[must_use]
    pub fn signed_hash(&self) -> [u8; 32] {
        self.signed_hash
    }

    #[must_use]
    pub fn access_list(&self) -> Option<&Vec<(Address, Vec<StorageKey>)>> {
        match &self.transaction {
            TransactionPayload::AccessList(AccessListTx { access_list, .. }) => Some(access_list),
            TransactionPayload::Legacy(_) => None,
        }
    }

    pub fn use_gas_limit_multiplier(&mut self) {
        let gas_multiplier = U256::from(GAS_LIMIT_MULTIPLIER_NO_CHAINID);

        match &mut self.transaction {
            TransactionPayload::AccessList(AccessListTx { gas_limit, .. })
            | TransactionPayload::Legacy(LegacyTx { gas_limit, .. }) => {
                *gas_limit = gas_limit.saturating_mul(gas_multiplier);
            }
        }
    }

    #[maybe_async]
    pub async fn validate(
        &self,
        origin: Address,
        backend: &impl AccountStorage,
    ) -> Result<(), crate::error::Error> {
        let chain_id = self
            .chain_id()
            .unwrap_or_else(|| backend.default_chain_id());

        if !backend.is_valid_chain_id(chain_id) {
            return Err(Error::InvalidChainId(chain_id));
        }

        let origin_nonce = backend.nonce(origin, chain_id).await;
        if origin_nonce != self.nonce() {
            let error = Error::InvalidTransactionNonce(origin, origin_nonce, self.nonce());
            return Err(error);
        }

        Ok(())
    }
}

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
