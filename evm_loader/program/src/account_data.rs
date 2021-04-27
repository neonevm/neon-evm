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
}

#[derive(Debug,Clone)]
pub struct Contract {
    pub owner: Pubkey,
    pub code_size: u32,
}

#[derive(Debug,Clone)]
pub enum AccountData {
    Account(Account),
    Contract(Contract),
    Empty
}

impl AccountData {
    pub const EMPTY_TAG: u8 = 0;

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(ProgramError::InvalidAccountData)?;
        Ok(match tag {
            AccountData::EMPTY_TAG => AccountData::Empty,
            Account::TAG => AccountData::Account( Account::unpack(rest)? ),
            Contract::TAG => AccountData::Contract( Contract::unpack(rest)? ),

            _ => return Err(ProgramError::InvalidAccountData),
        })
    }
}

impl Account {
    pub const TAG: u8 = 1;
    pub const HEADER_SIZE: usize = 20+1+8+32+32;
    pub const SIZE: usize = 1 + Account::HEADER_SIZE;

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() < Account::HEADER_SIZE {
            return Err(ProgramError::InvalidAccountData);
        }
        let data = array_ref![input, 0, Account::HEADER_SIZE];
        let (ether, nonce, trx_count, signer, code_account) = array_refs![data, 20, 1, 8, 32, 32];
        
        Ok(
            Account {
                ether: H160::from_slice(&*ether),
                nonce: nonce[0],
                trx_count: u64::from_le_bytes(*trx_count),
                signer: Pubkey::new_from_array(*signer),
                code_account: Pubkey::new_from_array(*code_account),
            }
        )
    }

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < Account::SIZE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        if dst[0] != AccountData::EMPTY_TAG && dst[0] != Account::TAG {
            return Err(ProgramError::InvalidAccountData);
        }
        dst[0] = Account::TAG;
        let data = array_mut_ref![dst, 1, Account::HEADER_SIZE];
        let (ether_dst, nonce_dst, trx_count_dst, signer_dst, code_account_dst) = 
                mut_array_refs![data, 20, 1, 8, 32, 32];
        *ether_dst = self.ether.to_fixed_bytes();
        nonce_dst[0] = self.nonce;
        *trx_count_dst = self.trx_count.to_le_bytes();
        signer_dst.copy_from_slice(self.signer.as_ref());
        code_account_dst.copy_from_slice(self.code_account.as_ref());
        Ok(Account::HEADER_SIZE)
    }
}

impl Contract {
    pub const TAG: u8 = 2;
    pub const HEADER_SIZE: usize = 32+4;
    pub const SIZE: usize = 1 + Contract::HEADER_SIZE;

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        if input.len() < Contract::HEADER_SIZE {
            return Err(ProgramError::InvalidAccountData);
        }
        let data = array_ref![input, 0, Contract::HEADER_SIZE];
        let (owner, code_size) = array_refs![data, 32, 4];
        Ok(
            Contract {
                owner: Pubkey::new_from_array(*owner),
                code_size: u32::from_le_bytes(*code_size),
            }
        )
    }

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < Contract::SIZE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        if dst[0] != AccountData::EMPTY_TAG && dst[0] != Contract::TAG {
            return Err(ProgramError::InvalidAccountData);
        }
        dst[0] = Contract::TAG;
        let data = array_mut_ref![dst, 1, Contract::HEADER_SIZE];
        let (owner_dst, code_size_dst) = 
                mut_array_refs![data, 32, 4];
                owner_dst.copy_from_slice(self.owner.as_ref());
        *code_size_dst = self.code_size.to_le_bytes();
        Ok(Contract::HEADER_SIZE)
    }
}


#[derive(Debug, Clone)]
pub struct Storage {
    pub caller: H160,
    pub nonce: u64,
    pub accounts_len: usize,
    pub executor_data_size: usize,
    pub evm_data_size: usize
}

impl Storage {
    pub const SIZE: usize = 20+8+8+8+8;

    pub fn unpack(src: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if src.len() < Storage::SIZE {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = array_ref![src, 0, Storage::SIZE];
        let (caller, nonce, accounts_len, executor_data_size, evm_data_size) = array_refs![data, 20, 8, 8, 8, 8];
        Ok((Self {
            caller: H160::from(*caller),
            nonce: u64::from_le_bytes(*nonce),
            accounts_len: usize::from_le_bytes(*accounts_len),
            executor_data_size: usize::from_le_bytes(*executor_data_size),
            evm_data_size: usize::from_le_bytes(*evm_data_size),
        }, &src[Storage::SIZE..]))
    }

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < Storage::SIZE {
            return Err(ProgramError::AccountDataTooSmall);
        }

        let data = array_mut_ref![dst, 0, Storage::SIZE];
        let (caller, nonce, accounts_len, executor_data_size, evm_data_size) = mut_array_refs![data, 20, 8, 8, 8, 8];
        *caller = self.caller.to_fixed_bytes();
        *nonce = self.nonce.to_le_bytes();
        *accounts_len = self.accounts_len.to_le_bytes();
        *executor_data_size = self.executor_data_size.to_le_bytes();
        *evm_data_size = self.evm_data_size.to_le_bytes();

        Ok(Storage::SIZE)
    }
}
