#![allow(clippy::module_name_repetitions)]
use std::{
    collections::HashMap,
    convert::TryFrom,
};

use solana_sdk::{
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
};

use crate::{ Config, Error, CommandResult };


fn read_elf_parameters(
        _config: &Config,
        program_data: &[u8],
    )-> HashMap<String, String>{
    let mut result = HashMap::new();
    let elf = goblin::elf::Elf::parse(program_data).expect("Unable to parse ELF file");
    elf.dynsyms.iter().for_each(|sym| {
        let name = String::from(&elf.dynstrtab[sym.st_name]);
        if name.starts_with("NEON")
        {
            let end = program_data.len();
            let from: usize = usize::try_from(sym.st_value).unwrap_or_else(|_| panic!("Unable to cast usize from u64:{:?}", sym.st_value));
            let to: usize = usize::try_from(sym.st_value + sym.st_size).unwrap_or_else(|err| panic!("Unable to cast usize from u64:{:?}. Error: {}", sym.st_value + sym.st_size, err));
            if to < end && from < end {
                let buf = &program_data[from..to];
                let value = std::str::from_utf8(buf).unwrap();
                result.insert(name, String::from(value));
            }
            else {
                panic!("{} is out of bounds", name);
            }
        }
    });

    result
}

pub fn read_elf_parameters_from_account(config: &Config) -> Result<HashMap<String, String>, Error> {
    let account = config.rpc_client
        .get_account_with_commitment(&config.evm_loader, config.commitment)?
        .value.ok_or(format!("Unable to find the account {}", &config.evm_loader))?;

    if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
        Ok(read_elf_parameters(config, &account.data))
    } else if account.owner == bpf_loader_upgradeable::id() {
        if let Ok(UpgradeableLoaderState::Program {
                      programdata_address,
                  }) = account.state()
        {
            let programdata_account = config.rpc_client
                .get_account_with_commitment(&programdata_address, config.commitment)?
                .value.ok_or(format!(
                "Failed to find associated ProgramData account {} for the program {}",
                programdata_address, &config.evm_loader))?;

            if let Ok(UpgradeableLoaderState::ProgramData { .. }) = programdata_account.state() {
                let offset =
                    UpgradeableLoaderState::programdata_data_offset().unwrap_or(0);
                let program_data = &programdata_account.data[offset..];
                Ok(read_elf_parameters(config, program_data))
            } else {
                Err(
                    format!("Invalid associated ProgramData account {} found for the program {}",
                            programdata_address, &config.evm_loader)
                        .into(),
                )
            }

        } else if let Ok(UpgradeableLoaderState::Buffer { .. }) = account.state() {
            let offset = UpgradeableLoaderState::buffer_data_offset().unwrap_or(0);
            let program_data = &account.data[offset..];
            Ok(read_elf_parameters(config, program_data))
        } else {
            Err(format!(
                "{} is not an upgradeble loader buffer or program account",
                &config.evm_loader
            )
                .into())
        }
    } else {
        Err(format!("{} is not a BPF program", &config.evm_loader).into())
    }

}

fn print_elf_parameters(params: &HashMap<String, String>){
    for (key, value) in params {
        println!("{}={}", key, value);
    }
}

fn read_program_data_from_file(config: &Config,
                               program_location: &str) -> CommandResult {
    let program_data = crate::read_program_data(program_location)?;
    let program_data = &program_data[..];
    let elf_params = read_elf_parameters(config, program_data);
    print_elf_parameters(&elf_params);
    Ok(())
}

fn read_program_data_from_account(config: &Config) {
    let elf_params = read_elf_parameters_from_account(config).unwrap();
    print_elf_parameters(&elf_params);
}

pub fn command_neon_elf(
    config: &Config,
    program_location: Option<&str>,
) -> CommandResult {
    program_location.map_or_else(
        || {read_program_data_from_account(config); Ok(())},
        |program_location| read_program_data_from_file(config, program_location),
    )
}
