use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use evm::{H160, U256};
use solana_program::pubkey::Pubkey;
use super::Packable;

/// Ethereum ERC20 allowance data account
// TODO remove all fields beside `value`
#[derive(Default, Debug)]
pub struct Data {
    /// Token owner
    pub owner: H160,
    /// Token spender
    pub spender: H160,
    /// Token contract
    pub contract: H160,
    /// Token mint
    pub mint: Pubkey,
    /// Amount
    pub value: U256,
}

impl Packable for Data {
    /// Allowance struct tag
    const TAG: u8 = super::TAG_ERC20_ALLOWANCE;
    /// Allowance struct serialized size
    const SIZE: usize = 20 + 20 + 20 + 32 + 32;

    /// Deserialize `ERC20Allowance` struct from input data
    #[must_use]
    fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, Data::SIZE];
        let (owner, spender, contract, mint, value) = array_refs![data, 20, 20, 20, 32, 32];

        Self {
            owner: H160::from(*owner),
            spender: H160::from(*spender),
            contract: H160::from(*contract),
            mint: Pubkey::new_from_array(*mint),
            value: U256::from_little_endian(value),
        }
    }

    /// Serialize `ERC20Allowance` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (owner, spender, contract, mint, value) = mut_array_refs![data, 20, 20, 20, 32, 32];

        *owner = self.owner.to_fixed_bytes();
        *spender = self.spender.to_fixed_bytes();
        *contract = self.contract.to_fixed_bytes();
        mint.copy_from_slice(self.mint.as_ref());
        self.value.to_little_endian(value);
    }
}
