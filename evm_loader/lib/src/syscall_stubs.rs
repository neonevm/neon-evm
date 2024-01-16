use log::info;
use solana_sdk::{program_error::ProgramError, program_stubs::SyscallStubs, sysvar::rent::Rent};

use crate::{errors::NeonError, rpc::Rpc};

pub struct EmulatorStubs {
    rent: Rent,
}

impl EmulatorStubs {
    pub async fn new(rpc: &impl Rpc) -> Result<Box<EmulatorStubs>, NeonError> {
        let rent_pubkey = solana_sdk::sysvar::rent::id();
        let data = rpc
            .get_account(&rent_pubkey)
            .await?
            .value
            .map(|a| a.data)
            .unwrap_or_default();
        let rent = bincode::deserialize(&data).map_err(|_| ProgramError::InvalidArgument)?;

        Ok(Box::new(Self { rent }))
    }
}

impl SyscallStubs for EmulatorStubs {
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

pub async fn setup_emulator_syscall_stubs(rpc: &impl Rpc) -> Result<(), NeonError> {
    let syscall_stubs = EmulatorStubs::new(rpc).await?;
    solana_sdk::program_stubs::set_syscall_stubs(syscall_stubs);

    Ok(())
}
