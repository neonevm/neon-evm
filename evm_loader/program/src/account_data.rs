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
    const EMPTY_TAG: u8 = 0;
    const ACCOUNT_TAG: u8 = 1;
    const CONTRACT_TAG: u8 = 2;

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input.split_first().ok_or(ProgramError::InvalidAccountData)?;
        Ok(match tag {
            AccountData::EMPTY_TAG => AccountData::Empty,
            AccountData::ACCOUNT_TAG => AccountData::Account( Account::unpack(rest) ),
            AccountData::CONTRACT_TAG => AccountData::Contract( Contract::unpack(rest) ),

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

            _ => return Err(ProgramError::InvalidAccountData),
        })
    }

    pub fn size(&self) -> usize {
        match self {
            AccountData::Account(acc) => acc.size() + 1,
            AccountData::Contract(acc) => acc.size() + 1,
            _ => return 1,
        }
    }
}

impl Account {
    pub const SIZE: usize = 20+1+8+32+32;

    pub fn unpack(input: &[u8]) -> Self {
        let data = array_ref![input, 0, Account::SIZE];
        let (ether, nonce, trx_count, signer, code_account) = array_refs![data, 20, 1, 8, 32, 32];

        Account {
            ether: H160::from_slice(&*ether),
            nonce: nonce[0],
            trx_count: u64::from_le_bytes(*trx_count),
            signer: Pubkey::new_from_array(*signer),
            code_account: Pubkey::new_from_array(*code_account),
        }
    }

    pub fn pack(acc: &Account, dst: &mut [u8]) -> usize {
        let data = array_mut_ref![dst, 0, Account::SIZE];
        let (ether_dst, nonce_dst, trx_count_dst, signer_dst, code_account_dst) = 
                mut_array_refs![data, 20, 1, 8, 32, 32];
        *ether_dst = acc.ether.to_fixed_bytes();
        nonce_dst[0] = acc.nonce;
        *trx_count_dst = acc.trx_count.to_le_bytes();
        signer_dst.copy_from_slice(acc.signer.as_ref());
        code_account_dst.copy_from_slice(acc.code_account.as_ref());
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