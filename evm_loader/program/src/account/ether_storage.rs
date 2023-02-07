use crate::{types::Address};
use super::{program, EthereumStorage, Operator, Packable, EthereumAccount};
use arrayref::{array_ref, array_refs, array_mut_ref, mut_array_refs};
use ethnum::U256;
use solana_program::{program_error::ProgramError, rent::Rent, sysvar::Sysvar, pubkey::Pubkey, account_info::AccountInfo};

/// Ethereum storage data account
#[derive(Default, Debug)]
pub struct Data {
    pub address: Address,
    pub generation: u32,
    pub index: U256,
}

impl Packable for Data {
    /// Storage struct tag
    const TAG: u8 = super::TAG_CONTRACT_STORAGE;
    /// Storage struct serialized size
    const SIZE: usize = 20 + 4 + 32;

    /// Deserialize `Storage` struct from input data
    #[must_use]
    fn unpack(input: &[u8]) -> Self {
        let data = array_ref![input, 0, Data::SIZE];
        let (address, generation, index) = array_refs![data, 20, 4, 32];

        Self {
            address: Address(*address),
            generation: u32::from_le_bytes(*generation),
            index: U256::from_le_bytes(*index),
        }
    }

    /// Serialize `Storage` struct into given destination
    fn pack(&self, output: &mut [u8]) {
        let data = array_mut_ref![output, 0, Data::SIZE];
        let (address, generation, index) = mut_array_refs![data, 20, 4, 32];
        
        *address = *self.address.as_bytes();
        *generation = self.generation.to_le_bytes();
        *index = self.index.to_le_bytes();
    }
}

pub struct EthereumStorageAddress {
    seed: [u8; 32],
    pubkey: Pubkey,
}

impl EthereumStorageAddress {
    #[must_use]
    fn make_seed(index: &U256) -> [u8; 32] {
        let mut buffer = [0_u8; 32];

        let index_bytes = index.to_be_bytes();
        let index_bytes = &index_bytes[3..31];

        for i in 0..28 {
            buffer[i] = index_bytes[i] & 0x7F;
        }

        #[allow(clippy::needless_range_loop)]
        for i in 0..7 {
            buffer[28] |= (index_bytes[i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[29] |= (index_bytes[7 + i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[30] |= (index_bytes[14 + i] & 0x80) >> (1 + i);
        }
        for i in 0..7 {
            buffer[31] |= (index_bytes[21 + i] & 0x80) >> (1 + i);
        }

        buffer
    }

    #[must_use]
    pub fn new(program_id: &Pubkey, base: &Pubkey, index: &U256) -> Self {
        let seed_buffer = Self::make_seed(index);
        let seed = unsafe { std::str::from_utf8_unchecked(&seed_buffer) };

        let pubkey = Pubkey::create_with_seed(base, seed, program_id).unwrap();

        Self { seed: seed_buffer, pubkey }
    }

    #[must_use]
    pub fn seed(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.seed) }
    }

    #[must_use]
    pub fn pubkey(&self) -> &Pubkey {
        &self.pubkey
    }
}

impl<'a> EthereumStorage<'a> {
    #[must_use]
    pub fn get(&self, subindex: u8) -> [u8; 32] {
        let data = self.info.data.borrow();
        let data = &data[Self::SIZE..];

        for chunk in data.chunks_exact(1 + 32) {
            if chunk[0] != subindex {
                continue;
            }

            return chunk[1..].try_into().unwrap();
        }

        [0_u8; 32]
    }

    pub fn set(
        &mut self,
        subindex: u8,
        value: &[u8; 32],
        operator: &Operator<'a>,
        system: &program::System<'a>,
    ) -> Result<(), ProgramError> {
        {
            let mut data = self.info.data.borrow_mut();
            let data = &mut data[Self::SIZE..];

            for chunk in data.chunks_exact_mut(1 + 32) {
                if chunk[0] != subindex {
                    continue;
                }

                chunk[1..].copy_from_slice(value);

                return Ok(());
            }
        } // drop `data`

        let new_len = self.info.data_len() + 1 + 32; // new_len <= 8.25 kb
        self.info.realloc(new_len, false)?;

        let minimum_balance = Rent::get()?.minimum_balance(new_len);
        if self.info.lamports() < minimum_balance {
            let required_lamports = minimum_balance - self.info.lamports();
            system.transfer(operator, self.info, required_lamports)?;
        }

        let mut data = self.info.data.borrow_mut();
        let data = &mut data[1..]; // skip tag

        let chunk_start = data.len() - 1 - 32;
        let chunk = &mut data[chunk_start..];

        chunk[0] = subindex;
        chunk[1..].copy_from_slice(value);

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create(
        contract: &EthereumAccount<'a>,
        storage_account: &'a AccountInfo<'a>,
        storage_address: &EthereumStorageAddress,
        index: U256,
        subindex: u8,
        value: &[u8; 32],
        operator: &Operator<'a>,
        system: &program::System<'a>,
    ) -> Result<Self, ProgramError> {
        let space = Self::SIZE + 1 + 32;

        system.create_account_with_seed(
            operator, contract, contract.info.owner, 
            storage_account, storage_address.seed(), 
            space
        )?;

        let storage = Self::init(storage_account, Data {
            address: contract.address,
            generation: contract.generation,
            index,
        })?;

        let mut data = storage_account.data.borrow_mut();
        let data = &mut data[Self::SIZE..];

        data[0] = subindex;
        data[1..].copy_from_slice(value);

        Ok(storage)
    }

    pub fn clear(
        &mut self,
        generation: u32,
        operator: &Operator<'a>,
    ) -> Result<(), ProgramError> {
        self.generation = generation;

        self.info.realloc(Self::SIZE, false)?;

        let minimum_balance = Rent::get()?.minimum_balance(Self::SIZE);
        let excessive_lamports = self.info.lamports().saturating_sub(minimum_balance);

        if excessive_lamports > 0 {
            **self.info.lamports.borrow_mut() -= excessive_lamports;
            **operator.lamports.borrow_mut() += excessive_lamports;
        }

        Ok(())
    }
}
