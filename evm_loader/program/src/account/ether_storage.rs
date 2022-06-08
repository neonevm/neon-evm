use arrayref::{array_mut_ref, array_ref};
use super::Packable;
use evm::U256;

/// Ethereum storage data account
#[derive(Default, Debug)]
pub struct Data {
    pub value: U256,
}

impl Packable for Data {
    /// Storage struct tag
    const TAG: u8 = super::TAG_CONTRACT_STORAGE;
    /// Storage struct serialized size
    const SIZE: usize = 32;

    /// Deserialize `Storage` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        let data = array_ref![src, 0, 32];
        Self { value: U256::from_big_endian(&data[..]) }
    }

    /// Serialize `Storage` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        let data = array_mut_ref![dst, 0, 32];
        self.value.to_big_endian(&mut data[..]);
    }
}
