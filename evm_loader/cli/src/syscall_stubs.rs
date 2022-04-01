use solana_sdk::{
    program_error::ProgramError, 
    program_stubs::SyscallStubs, 
    sysvar::rent::Rent
};

use crate::errors::NeonCliError;
use crate::Config;

pub struct Stubs {
    rent: Rent,
}

impl Stubs {
    pub fn new(config: &Config) -> Result<Box<Stubs>, NeonCliError> {
        let rent_pubkey = solana_sdk::sysvar::rent::id();
        let data = config.rpc_client.get_account_data(&rent_pubkey)?;
        let rent = bincode::deserialize(&data).map_err(|_| ProgramError::InvalidArgument)?;

        Ok(Box::new(Self { rent }))
    }
}

impl SyscallStubs for Stubs {
    fn sol_get_rent_sysvar(&self, pointer: *mut u8) -> u64 {
        unsafe {
            #[allow(clippy::cast_ptr_alignment)]
            let rent = pointer.cast::<Rent>();
            *rent = self.rent;
        }

        0
    }
}
