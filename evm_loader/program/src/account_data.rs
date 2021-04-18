use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use primitive_types::H160;
use solana_program::{
    program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(Debug,Clone)]
pub struct AccountData {
    pub ether: H160,
    pub nonce: u8,
    pub trx_count: u64,
    pub signer: Pubkey,
    pub code_account: Pubkey,
}

#[derive(Debug,Clone)]
pub struct ContractData {
    pub owner: Pubkey,
    pub code_size: u32,
}

#[derive(Debug,Clone)]
pub enum AccountType {
    AccountData(AccountData),
    ContractData(ContractData),
    Empty
}

impl AccountType {
    // pub fn size() -> usize {AccountData::SIZE}

    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        use ProgramError::InvalidAccountData;

        let (&tag, rest) = input.split_first().ok_or(InvalidAccountData)?;
        Ok(match tag {
            0 => {
                AccountType::Empty
            },
            1 => {
                let data = array_ref![rest, 0, AccountData::SIZE];
                let (ether, nonce, trx_count, signer, code_account) = array_refs![data, 20, 1, 8, 32, 32];
                
                AccountType::AccountData(
                    AccountData {
                        ether: H160::from_slice(&*ether),
                        nonce: nonce[0],
                        trx_count: u64::from_le_bytes(*trx_count),
                        signer: Pubkey::new_from_array(*signer),
                        code_account: Pubkey::new_from_array(*code_account),
                    }
                )
            },
            2 => {
                let data = array_ref![rest, 0, ContractData::SIZE];
                let (owner, code_size) = array_refs![data, 32, 4];
                AccountType::ContractData(
                    ContractData {
                        owner: Pubkey::new_from_array(*owner),
                        code_size: u32::from_le_bytes(*code_size),
                    }
                )
            },

            _ => return Err(InvalidAccountData),
        })
    }
}


impl AccountData {
    pub const SIZE: usize = 20+1+8+32+32;

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < AccountData::SIZE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let data = array_mut_ref![dst, 0, AccountData::SIZE];
        let (ether_dst, nonce_dst, trx_count_dst, signer_dst, code_account_dst) = 
                mut_array_refs![data, 20, 1, 8, 32, 32];
        *ether_dst = self.ether.to_fixed_bytes();
        nonce_dst[0] = self.nonce;
        *trx_count_dst = self.trx_count.to_le_bytes();
        signer_dst.copy_from_slice(self.signer.as_ref());
        code_account_dst.copy_from_slice(self.code_account.as_ref());
        Ok(AccountData::SIZE)
    }
}

impl ContractData {
    pub const SIZE: usize = 32+4;

    pub fn pack(&self, dst: &mut [u8]) -> Result<usize, ProgramError> {
        if dst.len() < ContractData::SIZE {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let data = array_mut_ref![dst, 0, ContractData::SIZE];
        let (owner_dst, code_size_dst) = 
                mut_array_refs![data, 32, 4];
                owner_dst.copy_from_slice(self.owner.as_ref());
        *code_size_dst = self.code_size.to_le_bytes();
        Ok(ContractData::SIZE)
    }
}