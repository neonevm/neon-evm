use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use primitive_types::H160;
use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(Debug,Clone)]
pub struct Account {
    pub ether: H160,
    pub nonce: u8,
    pub trx_count: u64,
    pub signer: Pubkey,
    pub code_account: Pubkey,
    pub blocked: Option<Pubkey>
}

#[derive(Debug,Clone)]
pub struct Contract {
    pub owner: Pubkey,
    pub code_size: u32,
}

#[derive(Debug,Clone)]
pub struct Storage {
    pub caller: H160,
    pub nonce: u64,
    pub accounts_len: usize,
    pub executor_data_size: usize,
    pub evm_data_size: usize
}

#[derive(Debug,Clone)]
pub enum AccountData {
    Account(Account),
    Contract(Contract),
    Storage(Storage),
    Empty
}

impl AccountData {
    const EMPTY_TAG: u8 = 0;
    const ACCOUNT_TAG: u8 = 1;
    const CONTRACT_TAG: u8 = 2;
    const STORAGE_TAG: u8 = 3;

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(ProgramError::InvalidAccountData)?;
        Ok(match tag {
            AccountData::EMPTY_TAG => AccountData::Empty,
            AccountData::ACCOUNT_TAG => AccountData::Account( Account::unpack(rest) ),
            AccountData::CONTRACT_TAG => AccountData::Contract( Contract::unpack(rest) ),
            AccountData::STORAGE_TAG => AccountData::Storage( Storage::unpack(rest) ),

            _ => return Err(ProgramError::InvalidAccountData),
        })
    }

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < 1 { return Err(ProgramError::AccountDataTooSmall); }
        Ok(match self {
            AccountData::Empty => 1,
            AccountData::Account(acc) => {
                if dst[0] != AccountData::ACCOUNT_TAG && dst[0] != AccountData::EMPTY_TAG { return Err(ProgramError::InvalidAccountData); }
                if dst.len() < self.size() { return Err(ProgramError::AccountDataTooSmall); }
                dst[0] = AccountData::ACCOUNT_TAG;
                Account::pack(acc, &mut dst[1..])
            },
            AccountData::Contract(acc) => {
                if dst[0] != AccountData::CONTRACT_TAG && dst[0] != AccountData::EMPTY_TAG { return Err(ProgramError::InvalidAccountData); }
                if dst.len() < self.size() { return Err(ProgramError::AccountDataTooSmall); }
                dst[0] = AccountData::CONTRACT_TAG;
                Contract::pack(acc, &mut dst[1..])
            },
            AccountData::Storage(acc) => {
                if dst[0] != AccountData::STORAGE_TAG && dst[0] != AccountData::EMPTY_TAG { return Err(ProgramError::InvalidAccountData); }
                if dst.len() < self.size() { return Err(ProgramError::AccountDataTooSmall); }
                dst[0] = AccountData::STORAGE_TAG;
                Storage::pack(acc, &mut dst[1..])
            },

            _ => return Err(ProgramError::InvalidAccountData),
        })
    }

    pub fn size(&self) -> usize {
        match self {
            AccountData::Account(acc) => acc.size() + 1,
            AccountData::Contract(acc) => acc.size() + 1,
            AccountData::Storage(acc) => acc.size() + 1,
            _ => return 1,
        }
    }

    pub fn get_account(&self) -> Result<&Account, ProgramError>  {
        match self {
            AccountData::Account(ref acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }

    pub fn get_mut_account(&mut self) -> Result<&mut Account, ProgramError>  {
        match self {
            AccountData::Account(ref mut acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }

    pub fn get_contract(&self) -> Result<&Contract, ProgramError>  {
        match self {
            AccountData::Contract(ref acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }

    pub fn get_mut_contract(&mut self) -> Result<&mut Contract, ProgramError>  {
        match self {
            AccountData::Contract(ref mut acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }

    pub fn get_storage(&self) -> Result<&Storage, ProgramError>  {
        match self {
            AccountData::Storage(ref acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }

    pub fn get_mut_storage(&mut self) -> Result<&mut Storage, ProgramError>  {
        match self {
            AccountData::Storage(ref mut acc) => Ok(acc),
            _ => return Err(ProgramError::InvalidAccountData),
        }
    }
}

impl Account {
    const SIZE: usize = 20+1+8+32+32+1+32;

    pub fn unpack(input: &[u8]) -> Self {
        let data = array_ref![input, 0, Account::SIZE];
        let (ether, nonce, trx_count, signer, code_account, is_blocked, blocked_by) = array_refs![data, 20, 1, 8, 32, 32, 1, 32];

        Account {
            ether: H160::from_slice(&*ether),
            nonce: nonce[0],
            trx_count: u64::from_le_bytes(*trx_count),
            signer: Pubkey::new_from_array(*signer),
            code_account: Pubkey::new_from_array(*code_account),
            blocked: if is_blocked[0] > 0 { Some(Pubkey::new_from_array(*blocked_by)) } else { None }
        }
    }

    pub fn pack(acc: &Account, dst: &mut [u8]) -> usize {
        let data = array_mut_ref![dst, 0, Account::SIZE];
        let (ether_dst, nonce_dst, trx_count_dst, signer_dst, code_account_dst, is_blocked_dst, blocked_by_dst) = 
                mut_array_refs![data, 20, 1, 8, 32, 32, 1, 32];
        *ether_dst = acc.ether.to_fixed_bytes();
        nonce_dst[0] = acc.nonce;
        *trx_count_dst = acc.trx_count.to_le_bytes();
        signer_dst.copy_from_slice(acc.signer.as_ref());
        code_account_dst.copy_from_slice(acc.code_account.as_ref());
        if let Some(blocked) = acc.blocked {
            is_blocked_dst[0] = 1;
            blocked_by_dst.copy_from_slice(blocked.as_ref());
        } else {
            is_blocked_dst[0] = 0;
        }

        Account::SIZE
    }

    pub fn size(&self) -> usize {
        Account::SIZE
    }
}

impl Contract {
    const SIZE: usize = 32+4;

    pub fn unpack(input: &[u8]) -> Self {
        let data = array_ref![input, 0, Contract::SIZE];
        let (owner, code_size) = array_refs![data, 32, 4];

        Contract {
            owner: Pubkey::new_from_array(*owner),
            code_size: u32::from_le_bytes(*code_size),
        }
    }

    pub fn pack(acc: &Contract, dst: &mut [u8]) -> usize {
        let data = array_mut_ref![dst, 0, Contract::SIZE];
        let (owner_dst, code_size_dst) = 
                mut_array_refs![data, 32, 4];
        owner_dst.copy_from_slice(acc.owner.as_ref());
        *code_size_dst = acc.code_size.to_le_bytes();
        Contract::SIZE
    }

    pub fn size(&self) -> usize {
        Contract::SIZE
    }
}

impl Storage {
    const SIZE: usize = 20+8+8+8+8;

    pub fn unpack(src: &[u8]) -> Self {
        let data = array_ref![src, 0, Storage::SIZE];
        let (caller, nonce, accounts_len, executor_data_size, evm_data_size) = array_refs![data, 20, 8, 8, 8, 8];
        
        Self {
            caller: H160::from(*caller),
            nonce: u64::from_le_bytes(*nonce),
            accounts_len: usize::from_le_bytes(*accounts_len),
            executor_data_size: usize::from_le_bytes(*executor_data_size),
            evm_data_size: usize::from_le_bytes(*evm_data_size),
        }
    }

    pub fn pack(&self, dst: &mut [u8]) -> usize {
        let data = array_mut_ref![dst, 0, Storage::SIZE];
        let (caller, nonce, accounts_len, executor_data_size, evm_data_size) = mut_array_refs![data, 20, 8, 8, 8, 8];
        *caller = self.caller.to_fixed_bytes();
        *nonce = self.nonce.to_le_bytes();
        *accounts_len = self.accounts_len.to_le_bytes();
        *executor_data_size = self.executor_data_size.to_le_bytes();
        *evm_data_size = self.evm_data_size.to_le_bytes();

        Storage::SIZE
    }

    pub fn size(&self) -> usize {
        Storage::SIZE
    }
}
