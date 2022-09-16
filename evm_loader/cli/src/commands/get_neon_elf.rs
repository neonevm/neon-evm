use std::{
    collections::HashMap,
    convert::TryFrom,
};

use solana_sdk::{
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
};

use crate::{ Config, errors::NeonCliError, NeonCliResult };

pub struct CachedElfParams {
    elf_params: HashMap<String,String>,
}
impl CachedElfParams {
    pub fn new(config: &Config) -> Self {
        Self {
            elf_params: read_elf_parameters_from_account(config).unwrap(),
        }
    }
    pub fn get(&self, param_name: &str) -> Option<&String> {
        self.elf_params.get(param_name)
    }
}

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

pub fn read_elf_parameters_from_account(config: &Config) -> Result<HashMap<String, String>, NeonCliError> {
    let account = config.rpc_client
        .get_account_with_commitment(&config.evm_loader, config.commitment)?
        .value.ok_or(NeonCliError::AccountNotFound(config.evm_loader))?;

    if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
        Ok(read_elf_parameters(config, &account.data))
    } else if account.owner == bpf_loader_upgradeable::id() {
        if let Ok(UpgradeableLoaderState::Program {
                      programdata_address,
                  }) = account.state()
        {
            let programdata_account = config.rpc_client
                .get_account_with_commitment(&programdata_address, config.commitment)?
                .value.ok_or(NeonCliError::AssociatedPdaNotFound(programdata_address,config.evm_loader))?;

            if let Ok(UpgradeableLoaderState::ProgramData { .. }) = programdata_account.state() {
                let offset = UpgradeableLoaderState::size_of_programdata_metadata();
                let program_data = &programdata_account.data[offset..];
                Ok(read_elf_parameters(config, program_data))
            } else {
                Err(NeonCliError::InvalidAssociatedPda(programdata_address,config.evm_loader))
            }

        } else if let Ok(UpgradeableLoaderState::Buffer { .. }) = account.state() {
            let offset = UpgradeableLoaderState::size_of_buffer_metadata();
            let program_data = &account.data[offset..];
            Ok(read_elf_parameters(config, program_data))
        } else {
            Err(NeonCliError::AccountIsNotUpgradeable(config.evm_loader))
        }
    } else {
        Err(NeonCliError::AccountIsNotBpf(config.evm_loader))
    }

}

fn print_elf_parameters(params: &HashMap<String, String>){
    for (key, value) in params {
        println!("{}={}", key, value);
    }
}

fn read_program_data_from_file(config: &Config,
                               program_location: &str) -> NeonCliResult {
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

pub fn execute(
    config: &Config,
    program_location: Option<&str>,
) -> NeonCliResult {
    program_location.map_or_else(
        || {read_program_data_from_account(config); Ok(())},
        |program_location| read_program_data_from_file(config, program_location),
    )
}
