use log::info;
use solana_sdk::{program_error::ProgramError, program_stubs::SyscallStubs, sysvar::rent::Rent};

use crate::{errors::NeonError, rpc::Rpc};

pub struct Stubs {
    rent: Rent,
}

impl Stubs {
    pub async fn new(rpc_client: &dyn Rpc) -> Result<Box<Stubs>, NeonError> {
        let rent_pubkey = solana_sdk::sysvar::rent::id();
        let data = rpc_client.get_account_data(&rent_pubkey).await?;
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

    fn sol_log(&self, message: &str) {
        info!("{}", message);
    }

    fn sol_log_data(&self, fields: &[&[u8]]) {
        let mut messages: Vec<String> = Vec::new();

        for f in fields {
            if let Ok(str) = String::from_utf8(f.to_vec()) {
                messages.push(str);
            } else {
                messages.push(hex::encode(f));
            }
        }

        info!("Program Data: {}", messages.join(" "));
    }
}
