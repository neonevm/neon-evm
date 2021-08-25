//! Structures stored in account data
use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use evm::H160;
use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Ethereum account data
#[derive(Debug,Clone)]
pub struct Account {
    /// Ethereum address
    pub ether: H160,
    /// Solana account nonce
    pub nonce: u8,
    /// Ethereum account nonce
    pub trx_count: u64,
    /// Address of solana account that stores code data (for contract accounts) of Pubkey([0_u8; 32]) if none
    pub code_account: Pubkey,
    /// Ethereum address
    pub blocked: Option<Pubkey>,
    /// ETH token account
    pub eth_token_account: Pubkey
}

/// Ethereum contract data account
#[derive(Debug,Clone)]
pub struct Contract {
    /// Solana account with ethereum account data associated with this code data
    pub owner: Pubkey,
    /// Contract code size
    pub code_size: u32,
}

/// Storage data account to store execution metainfo between steps for iterative execution
#[derive(Debug,Clone)]
pub struct Storage {
    /// Ethereum transaction caller address
    pub caller: H160,
    /// Ethereum transaction caller nonce
    pub nonce: u64,
    /// Ethereum transaction gas limit
    pub gas_limit: u64,
    /// Ethereum transaction gas price
    pub gas_price: u64,
    /// Last transaction slot
    pub slot: u64,
    /// Operator public key
    pub operator: Pubkey,
    /// Stored accounts length
    pub accounts_len: usize,
    /// Stored executor data size
    pub executor_data_size: usize,
    /// Stored evm data size
    pub evm_data_size: usize
}

/// Structured data stored in account data
#[derive(Debug,Clone)]
pub enum AccountData {
    /// Ethereum account data
    Account(Account),
    /// Ethereum contract data account
    Contract(Contract),
    /// Storage data
    Storage(Storage),
    /// Empty account data
    Empty
}

impl AccountData {
    const EMPTY_TAG: u8 = 0;
    const ACCOUNT_TAG: u8 = 1;
    const CONTRACT_TAG: u8 = 2;
    const STORAGE_TAG: u8 = 3;

    /// Unpack `AccountData` from Solana's account data
    /// ```
    /// let caller_info_data = AccountData::unpack(&account_info.data.borrow())?;
    /// match caller_info_data {
    ///     AccountData::Account(acc) => ...,
    ///     AccountData::Contract(acc) => ...,
    ///     AccountData::Storage(acc) => ...,
    ///     Empty => ...,
    /// }
    /// ```
    /// # Errors
    ///
    /// Will return `ProgramError::InvalidAccountData` if `input` cannot be
    /// parsed to `AccountData`
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or_else(||E!(ProgramError::InvalidAccountData))?;
        Ok(match tag {
            Self::EMPTY_TAG => Self::Empty,
            Self::ACCOUNT_TAG => Self::Account( Account::unpack(rest) ),
            Self::CONTRACT_TAG => Self::Contract( Contract::unpack(rest) ),
            Self::STORAGE_TAG => Self::Storage( Storage::unpack(rest) ),

            _ => return Err!(ProgramError::InvalidAccountData; "tag={:?}", tag),
        })
    }


    /// Serialize `AccountData` into Solana's account data
    /// ```
    /// let contract_data = AccountData::Contract( Contract {owner: *account_info.key, code_size: 0_u32} );
    /// contract_data.pack(&mut program_code.data.borrow_mut())?;
    /// ```
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::AccountDataTooSmall` if `dst` size not enough to store serialized `AccountData`
    /// `ProgramError::InvalidAccountData` if in `dst` stored incompatible `AccountData` struct
    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.is_empty() {return Err!(ProgramError::AccountDataTooSmall; "dst={:?}", dst);}
        Ok(match self {
            Self::Empty => {
                if dst.len() < self.size() {
                    return Err!(ProgramError::AccountDataTooSmall; "dst.len()={:?}, self.size()={:?}", dst.len(), self.size());
                }
                dst[0] = Self::EMPTY_TAG;
                (Self::Empty).size()
            },
            Self::Account(acc) => {
                if dst[0] != Self::ACCOUNT_TAG && dst[0] != Self::EMPTY_TAG {
                    return Err!(ProgramError::InvalidAccountData; "dst[0]={:?}", dst[0]);
                }
                if dst.len() < self.size() { return Err!(ProgramError::AccountDataTooSmall; "dst.len()={:?} < self.size()={:?}", dst.len(), self.size()); }
                dst[0] = Self::ACCOUNT_TAG;
                Account::pack(acc, &mut dst[1..])
            },
            Self::Contract(acc) => {
                if dst[0] != Self::CONTRACT_TAG && dst[0] != Self::EMPTY_TAG { return Err!(ProgramError::InvalidAccountData; "dst[0]={:?}", dst[0]); }
                if dst.len() < self.size() { return Err!(ProgramError::AccountDataTooSmall; "dst.len()={:?} < self.size()={:?}", dst.len(), self.size()); }
                dst[0] = Self::CONTRACT_TAG;
                Contract::pack(acc, &mut dst[1..])
            },
            Self::Storage(acc) => {
                if dst[0] != Self::STORAGE_TAG && dst[0] != Self::EMPTY_TAG { return Err!(ProgramError::InvalidAccountData; "dst[0]={:?}", dst[0]); }
                if dst.len() < self.size() { return Err!(ProgramError::AccountDataTooSmall; "dst.len()={:?} < self.size()={:?}", dst.len(), self.size()); }
                dst[0] = Self::STORAGE_TAG;
                Storage::pack(acc, &mut dst[1..])
            },
        })
    }

    /// Get `AccountData` struct size
    #[must_use]
    pub const fn size(&self) -> usize {
        match self {
            Self::Account(_acc) => Account::size() + 1,
            Self::Contract(_acc) => Contract::size() + 1,
            Self::Storage(_acc) => Storage::size() + 1,
            Self::Empty => 1,
        }
    }

    /// Get `Account` struct  reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Account` struct
    pub const fn get_account(&self) -> Result<&Account, ProgramError>  {
        match self {
            Self::Account(ref acc) => Ok(acc),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    /// Get mutable `Account` struct reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Account` struct
    pub fn get_mut_account(&mut self) -> Result<&mut Account, ProgramError>  {
        match self {
            Self::Account(ref mut acc) => Ok(acc),
            _ => Err!(ProgramError::InvalidAccountData),
        }
    }

    /// Get `Contract` struct  reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Contract` struct
    pub const fn get_contract(&self) -> Result<&Contract, ProgramError>  {
        match self {
            Self::Contract(ref acc) => Ok(acc),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    /// Get mutable `Contract` struct reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Contract` struct
    pub fn get_mut_contract(&mut self) -> Result<&mut Contract, ProgramError>  {
        match self {
            Self::Contract(ref mut acc) => Ok(acc),
            _ => Err!(ProgramError::InvalidAccountData),
        }
    }

    /// Get `Storage` struct  reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Storage` struct
    pub const fn get_storage(&self) -> Result<&Storage, ProgramError>  {
        match self {
            Self::Storage(ref acc) => Ok(acc),
            _ => Err(ProgramError::InvalidAccountData),
        }
    }

    /// Get mutable `Storage` struct reference from `AccountData` if it is stored in
    /// # Errors
    ///
    /// Will return:
    /// `ProgramError::InvalidAccountData` if doesn't contain `Storage` struct
    pub fn get_mut_storage(&mut self) -> Result<&mut Storage, ProgramError>  {
        match self {
            Self::Storage(ref mut acc) => Ok(acc),
            _ => Err!(ProgramError::InvalidAccountData),
        }
    }
}

impl Account {
    /// Account struct serialized size
    pub const SIZE: usize = 20+1+8+32+1+32+32;

    /// Deserialize `Account` struct from input data
    #[must_use]
    pub fn unpack(input: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![input, 0, Account::SIZE];
        let (ether, nonce, trx_count, code_account, is_blocked, blocked_by, eth) = array_refs![data, 20, 1, 8, 32, 1, 32, 32];

        Self {
            ether: H160::from_slice(&*ether),
            nonce: nonce[0],
            trx_count: u64::from_le_bytes(*trx_count),
            code_account: Pubkey::new_from_array(*code_account),
            blocked: if is_blocked[0] > 0 { Some(Pubkey::new_from_array(*blocked_by)) } else { None },
            eth_token_account: Pubkey::new_from_array(*eth)
        }
    }

    /// Serialize `Account` struct into given destination
    pub fn pack(acc: &Self, dst: &mut [u8]) -> usize {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Account::SIZE];
        let (ether_dst, nonce_dst, trx_count_dst, code_account_dst, is_blocked_dst, blocked_by_dst, eth_dst) = 
                mut_array_refs![data, 20, 1, 8, 32, 1, 32, 32];
        *ether_dst = acc.ether.to_fixed_bytes();
        nonce_dst[0] = acc.nonce;
        *trx_count_dst = acc.trx_count.to_le_bytes();
        code_account_dst.copy_from_slice(acc.code_account.as_ref());
        if let Some(blocked) = acc.blocked {
            is_blocked_dst[0] = 1;
            blocked_by_dst.copy_from_slice(blocked.as_ref());
        } else {
            is_blocked_dst[0] = 0;
        }
        eth_dst.copy_from_slice(acc.eth_token_account.as_ref());

        Self::SIZE
    }

    /// Get `Account` struct size
    #[must_use]
    pub const fn size() -> usize {
        Self::SIZE
    }
}

impl Contract {
    /// Contract struct serialized size
    pub const SIZE: usize = 32+4;

    /// Deserialize `Contract` struct from input data
    #[must_use]
    pub fn unpack(input: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![input, 0, Contract::SIZE];
        let (owner, code_size) = array_refs![data, 32, 4];

        Self {
            owner: Pubkey::new_from_array(*owner),
            code_size: u32::from_le_bytes(*code_size),
        }
    }

    /// Serialize `Contract` struct into given destination
    pub fn pack(acc: &Self, dst: &mut [u8]) -> usize {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Contract::SIZE];
        let (owner_dst, code_size_dst) = 
                mut_array_refs![data, 32, 4];
        owner_dst.copy_from_slice(acc.owner.as_ref());
        *code_size_dst = acc.code_size.to_le_bytes();
        Self::SIZE
    }

    /// Get `Contract` struct size
    #[must_use]
    pub const fn size() -> usize {
        Self::SIZE
    }
}

impl Storage {
    /// Storage struct serialized size
    const SIZE: usize = 20+8+8+8+8+32+8+8+8;

    /// Deserialize `Storage` struct from input data
    #[must_use]
    pub fn unpack(src: &[u8]) -> Self {
        #[allow(clippy::use_self)]
        let data = array_ref![src, 0, Storage::SIZE];
        let (caller, nonce, gas_limit, gas_price, slot, operator, accounts_len, executor_data_size, evm_data_size) = array_refs![data, 20, 8, 8, 8, 8, 32, 8, 8, 8];
        
        Self {
            caller: H160::from(*caller),
            nonce: u64::from_le_bytes(*nonce),
            gas_limit: u64::from_le_bytes(*gas_limit),
            gas_price: u64::from_le_bytes(*gas_price),
            slot: u64::from_le_bytes(*slot),
            operator: Pubkey::new_from_array(*operator),
            accounts_len: usize::from_le_bytes(*accounts_len),
            executor_data_size: usize::from_le_bytes(*executor_data_size),
            evm_data_size: usize::from_le_bytes(*evm_data_size),
        }
    }

    /// Serialize `Storage` struct into given destination
    pub fn pack(&self, dst: &mut [u8]) -> usize {
        #[allow(clippy::use_self)]
        let data = array_mut_ref![dst, 0, Storage::SIZE];
        let (caller, nonce, gas_limit, gas_price, slot, operator, accounts_len, executor_data_size, evm_data_size) = mut_array_refs![data, 20, 8, 8, 8, 8, 32, 8, 8, 8];
        *caller = self.caller.to_fixed_bytes();
        *nonce = self.nonce.to_le_bytes();
        *gas_limit = self.gas_limit.to_le_bytes();
        *gas_price = self.gas_price.to_le_bytes();
        *slot = self.slot.to_le_bytes();
        operator.copy_from_slice(self.operator.as_ref());
        *accounts_len = self.accounts_len.to_le_bytes();
        *executor_data_size = self.executor_data_size.to_le_bytes();
        *evm_data_size = self.evm_data_size.to_le_bytes();

        Self::SIZE
    }

    /// Get `Storage` struct size
    #[must_use]
    pub const fn size() -> usize {
        Self::SIZE
    }
}
