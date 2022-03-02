use super::Packable;
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use evm::{H160, U256};
use solana_program::entrypoint::ProgramResult;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

/// Ethereum account data v1
#[deprecated]
#[derive(Debug)]
pub struct DataV1 {
    /// Ethereum address
    pub ether: H160,
    /// Solana account nonce
    pub nonce: u8,
    /// Ethereum account nonce
    pub trx_count: u64,
    /// Address of solana account that stores code data (for contract accounts) or Pubkey([0_u8; 32]) if none
    pub code_account: Pubkey,
    /// Public key of storage account, associated with the transaction that locked this account for writing
    pub rw_blocked_acc: Option<Pubkey>,
    /// ETH token account
    pub eth_token_account: Pubkey,
    /// counter of the read-only locks
    pub ro_blocked_cnt: u8,
}

/// Ethereum account data v2
#[derive(Debug)]
pub struct Data {
    /// Ethereum address
    pub address: H160,
    /// Solana account nonce
    pub bump_seed: u8,
    /// Ethereum account nonce
    pub trx_count: u64,
    /// Neon token balance
    pub balance: U256,
    /// Address of solana account that stores code data (for contract accounts)
    pub code_account: Option<Pubkey>,
    /// Read-write lock
    pub rw_blocked: bool,
    /// Read-only lock counter
    pub ro_blocked_count: u8,
}

impl Data {
    pub fn check_blocked(&self, required_exclusive_access: bool) -> ProgramResult {
        if self.rw_blocked {
            // error message is parsed in proxy, do not change
            return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on rw locked account {}", self.address);
        }
        if required_exclusive_access && self.ro_blocked_count > 0 {
            return Err!(ProgramError::InvalidAccountData; "trying to execute transaction on ro locked account {}", self.address);
        }

        Ok(())
    }
}


#[allow(deprecated)]
impl Packable for DataV1 {
    /// Account struct tag
    const TAG: u8 = super::TAG_ACCOUNT_V1;
    /// Account struct serialized size
    const SIZE: usize = 20 + 1 + 8 + 32 + 1 + 32 + 32 + 1;

    /// Deserialize `Account` struct from input data
    #[must_use]
    fn unpack(input: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![input, 0, DataV1::SIZE];
        let (
            ether,
            nonce,
            trx_count,
            code_account,
            is_rw_blocked,
            rw_blocked_by,
            eth,
            ro_blocked_cnt,
        ) = array_refs![data, 20, 1, 8, 32, 1, 32, 32, 1];

        Self {
            ether: H160::from_slice(&*ether),
            nonce: nonce[0],
            trx_count: u64::from_le_bytes(*trx_count),
            code_account: Pubkey::new_from_array(*code_account),
            rw_blocked_acc: if is_rw_blocked[0] > 0 {
                Some(Pubkey::new_from_array(*rw_blocked_by))
            } else {
                None
            },
            eth_token_account: Pubkey::new_from_array(*eth),
            ro_blocked_cnt: ro_blocked_cnt[0],
        }
    }

    /// Serialize `Account` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, DataV1::SIZE];
        let (
            ether_dst,
            nonce_dst,
            trx_count_dst,
            code_account_dst,
            is_rw_blocked_dst,
            rw_blocked_by_dst,
            eth_dst,
            ro_blocked_cnt_dst,
        ) = mut_array_refs![data, 20, 1, 8, 32, 1, 32, 32, 1];

        *ether_dst = self.ether.to_fixed_bytes();
        nonce_dst[0] = self.nonce;
        *trx_count_dst = self.trx_count.to_le_bytes();
        code_account_dst.copy_from_slice(self.code_account.as_ref());
        if let Some(blocked_acc) = self.rw_blocked_acc {
            is_rw_blocked_dst[0] = 1;
            rw_blocked_by_dst.copy_from_slice(blocked_acc.as_ref());
        } else {
            is_rw_blocked_dst[0] = 0;
        }
        eth_dst.copy_from_slice(self.eth_token_account.as_ref());
        ro_blocked_cnt_dst[0] = self.ro_blocked_cnt;
    }
}

impl Packable for Data {
    /// `AccountV2` struct tag
    const TAG: u8 = super::TAG_ACCOUNT;
    /// `AccountV2` struct serialized size
    const SIZE: usize = 20 + 1 + 8 + 32 + 32 + 1 + 1;

    /// Deserialize `AccountV2` struct from input data
    #[must_use]
    fn unpack(input: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![input, 0, Data::SIZE];
        let (
            address,
            bump_seed,
            trx_count,
            balance,
            code_account,
            rw_blocked,
            ro_blocked_count,
        ) = array_refs![data, 20, 1, 8, 32, 32, 1, 1];

        Self {
            address: H160::from_slice(address),
            bump_seed: bump_seed[0],
            trx_count: u64::from_le_bytes(*trx_count),
            balance: U256::from_little_endian(balance),
            code_account: if *code_account == [0_u8; 32] {
                None
            } else {
                Some(Pubkey::new(code_account))
            },
            rw_blocked: (rw_blocked[0] > 0),
            ro_blocked_count: ro_blocked_count[0],
        }
    }

    /// Serialize `AccountV2` struct into given destination
    fn pack(&self, dst: &mut [u8]) {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Data::SIZE];
        let (
            address,
            bump_seed,
            trx_count,
            balance,
            code_account,
            rw_blocked,
            ro_blocked_count
        ) = mut_array_refs![data, 20, 1, 8, 32, 32, 1, 1];

        *address = self.address.to_fixed_bytes();
        bump_seed[0] = self.bump_seed;
        *trx_count = self.trx_count.to_le_bytes();
        self.balance.to_little_endian(balance);
        if let Some(key) = self.code_account {
            code_account.copy_from_slice(key.as_ref());
        } else {
            code_account.fill(0_u8);
        }
        rw_blocked[0] = u8::from(self.rw_blocked);
        ro_blocked_count[0] = self.ro_blocked_count;
    }
}
