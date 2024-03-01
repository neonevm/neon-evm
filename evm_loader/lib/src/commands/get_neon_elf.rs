use anyhow::{Context as AContext, Result};
use solana_sdk::{
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    pubkey::Pubkey,
};
use std::{collections::HashMap, convert::TryFrom, fs::File, io::Read};

use crate::rpc::Rpc;
use crate::{errors::NeonError, Config, NeonResult};

pub type GetNeonElfReturn = HashMap<String, String>;

pub struct CachedElfParams {
    elf_params: GetNeonElfReturn,
}

impl CachedElfParams {
    pub async fn new(config: &Config, rpc: &impl Rpc) -> Self {
        Self {
            elf_params: read_elf_parameters_from_account(config, rpc)
                .await
                .expect("read elf_params error"),
        }
    }
    pub fn get(&self, param_name: &str) -> Option<&String> {
        self.elf_params.get(param_name)
    }
}

pub fn read_elf_parameters(_config: &Config, program_data: &[u8]) -> GetNeonElfReturn {
    let mut result = HashMap::new();
    let elf = goblin::elf::Elf::parse(program_data).expect("Unable to parse ELF file");
    let ctx = goblin::container::Ctx::new(
        if elf.is_64 {
            goblin::container::Container::Big
        } else {
            goblin::container::Container::Little
        },
        if elf.little_endian {
            scroll::Endian::Little
        } else {
            scroll::Endian::Big
        },
    );

    let (num_syms, offset) = elf
        .section_headers
        .into_iter()
        .find(|section| section.sh_type == goblin::elf::section_header::SHT_DYNSYM)
        .map(|section| (section.sh_size / section.sh_entsize, section.sh_offset))
        .unwrap();
    let dynsyms = goblin::elf::Symtab::parse(
        program_data,
        offset.try_into().expect("Offset too large"),
        num_syms.try_into().expect("Count too large"),
        ctx,
    )
    .unwrap();
    dynsyms.iter().for_each(|sym| {
        let name = String::from(&elf.dynstrtab[sym.st_name]);
        if name.starts_with("NEON") {
            let end = program_data.len();
            let from: usize = usize::try_from(sym.st_value)
                .unwrap_or_else(|_| panic!("Unable to cast usize from u64:{:?}", sym.st_value));
            let to: usize = usize::try_from(sym.st_value + sym.st_size).unwrap_or_else(|err| {
                panic!(
                    "Unable to cast usize from u64:{:?}. Error: {err}",
                    sym.st_value + sym.st_size
                )
            });
            if to < end && from < end {
                let buf = &program_data[from..to];
                let value = std::str::from_utf8(buf).expect("read elf value error");
                result.insert(name, String::from(value));
            } else {
                panic!("{name} is out of bounds");
            }
        }
    });

    result
}

pub fn get_elf_parameter(data: &[u8], elf_parameter: &str) -> Result<String> {
    let offset = UpgradeableLoaderState::size_of_programdata_metadata();

    // Check if the offset is within the bounds of `data`
    if data.len() <= offset {
        let error_msg = format!(
            "Offset beyond data bounds. Data len: {}, offset: {offset}, data bytes: {data:?}",
            data.len(),
        );
        return Err(anyhow::anyhow!(error_msg));
    }

    let program_data = &data[offset..];

    let elf = goblin::elf::Elf::parse(program_data).context("Unable to parse ELF file")?;
    let ctx = goblin::container::Ctx::new(
        if elf.is_64 {
            goblin::container::Container::Big
        } else {
            goblin::container::Container::Little
        },
        if elf.little_endian {
            scroll::Endian::Little
        } else {
            scroll::Endian::Big
        },
    );

    let (num_syms, offset) = elf
        .section_headers
        .into_iter()
        .find(|section| section.sh_type == goblin::elf::section_header::SHT_DYNSYM)
        .map(|section| (section.sh_size / section.sh_entsize, section.sh_offset))
        .ok_or_else(|| anyhow::anyhow!("SHT_DYNSYM section not found"))?;

    let dynsyms = goblin::elf::Symtab::parse(
        program_data,
        offset.try_into().context("Offset too large")?,
        num_syms.try_into().context("Count too large")?,
        ctx,
    )
    .context("Error parsing Symtab")?;

    for sym in dynsyms.iter() {
        let name = &elf.dynstrtab[sym.st_name];
        if name == elf_parameter {
            let end = program_data.len();
            let from: usize = usize::try_from(sym.st_value)
                .map_err(|_| anyhow::anyhow!("Unable to cast usize from u64:{:?}", sym.st_value))?;
            let to: usize = usize::try_from(sym.st_value + sym.st_size).map_err(|err| {
                anyhow::anyhow!(
                    "Unable to cast usize from u64:{:?}. Error: {err}",
                    sym.st_value + sym.st_size
                )
            })?;

            if to < end && from < end {
                let buf = &program_data[from..to];
                let value = std::str::from_utf8(buf).context("Read ELF value error")?;
                return Ok(String::from(value));
            } else {
                return Err(anyhow::anyhow!("{name} is out of bounds"));
            }
        }
    }

    Err(anyhow::anyhow!("ELF parameter not found"))
}

pub async fn read_elf_parameters_from_account(
    config: &Config,
    rpc: &impl Rpc,
) -> Result<GetNeonElfReturn, NeonError> {
    let (_, program_data) = read_program_data_from_account(config, rpc, &config.evm_loader).await?;
    Ok(read_elf_parameters(config, &program_data))
}

pub async fn read_program_data_from_account(
    config: &Config,
    rpc: &impl Rpc,
    evm_loader: &Pubkey,
) -> Result<(Option<Pubkey>, Vec<u8>), NeonError> {
    let account = rpc
        .get_account_with_commitment(evm_loader, config.commitment)
        .await?
        .value
        .ok_or(NeonError::AccountNotFound(*evm_loader))?;

    if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
        Ok((None, account.data))
    } else if account.owner == bpf_loader_upgradeable::id() {
        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = account.state()
        {
            let programdata_account = rpc
                .get_account_with_commitment(&programdata_address, config.commitment)
                .await?
                .value
                .ok_or(NeonError::AssociatedPdaNotFound(
                    programdata_address,
                    config.evm_loader,
                ))?;

            if let Ok(UpgradeableLoaderState::ProgramData {
                upgrade_authority_address,
                ..
            }) = programdata_account.state()
            {
                let offset = UpgradeableLoaderState::size_of_programdata_metadata();
                let program_data = &programdata_account.data[offset..];
                Ok((upgrade_authority_address, program_data.to_vec()))
            } else {
                Err(NeonError::InvalidAssociatedPda(
                    programdata_address,
                    config.evm_loader,
                ))
            }
        } else if let Ok(UpgradeableLoaderState::Buffer {
            authority_address, ..
        }) = account.state()
        {
            let offset = UpgradeableLoaderState::size_of_buffer_metadata();
            let program_data = &account.data[offset..];
            Ok((authority_address, program_data.to_vec()))
        } else {
            Err(NeonError::AccountIsNotUpgradeable(config.evm_loader))
        }
    } else {
        Err(NeonError::AccountIsNotBpf(config.evm_loader))
    }
}

/// # Errors
pub fn read_program_data(program_location: &str) -> Result<Vec<u8>, NeonError> {
    let mut file = File::open(program_location)?;
    let mut program_data = Vec::new();
    file.read_to_end(&mut program_data)?;
    Ok(program_data)
}

fn read_program_params_from_file(
    config: &Config,
    program_location: &str,
) -> NeonResult<GetNeonElfReturn> {
    let program_data = read_program_data(program_location)?;
    Ok(read_elf_parameters(config, &program_data))
}

async fn read_program_params_from_account(
    config: &Config,
    rpc: &impl Rpc,
) -> NeonResult<GetNeonElfReturn> {
    read_elf_parameters_from_account(config, rpc).await
}

pub async fn execute(
    config: &Config,
    rpc: &impl Rpc,
    program_location: Option<&str>,
) -> NeonResult<GetNeonElfReturn> {
    if let Some(program_location) = program_location {
        read_program_params_from_file(config, program_location)
    } else {
        read_program_params_from_account(config, rpc).await
    }
}
