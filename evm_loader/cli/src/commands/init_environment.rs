use {
    crate::{
        read_program_data,
        neon_cli_revision,
        Config,
        errors::NeonCliError,
        commands::{
            get_neon_elf::{read_elf_parameters, read_program_data_from_account},
            transaction_executor::TransactionExecutor,
        },
    },
    spl_associated_token_account::get_associated_token_address,
    log::{info, warn, error},
    solana_sdk::{
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        bpf_loader_upgradeable,
        system_program,
        signer::Signer,
        program_pack::Pack,
        system_instruction,
        signer::keypair::{Keypair,read_keypair_file},
    },
    spl_token::{self, native_mint},
    evm_loader::{
        account::{MainTreasury, Treasury},
        config::TREASURY_POOL_SEED,
    },
    std::collections::HashMap,
    std::path::Path,
    thiserror::Error,
};

struct Parameters {
    params: HashMap<String,String>
}

impl Parameters {
    pub fn new(params: HashMap<String,String>) -> Self {
        Self {params}
    }

    pub fn get<T: std::str::FromStr>(&self, name: &str) -> Result<T,NeonCliError> {
        self.params.get(name)
            .ok_or_else(|| EnvironmentError::MissingProgramParameter(name.to_string()))?
            .parse::<T>().map_err(|_| EnvironmentError::InvalidProgramParameter(name.to_string()).into())
    }
}

#[derive(Debug,Error)]
pub enum EnvironmentError {
    #[error("NeonEVM and CLI revisions mismatch {0:?} != {1:?}")]
    RevisionMismatch(String,String),

    #[error("Missing parameter '{0:?}' in ELF file")]
    MissingProgramParameter(String),

    #[error("Invalid parameter '{0:?}' in ELF file")]
    InvalidProgramParameter(String),

    #[error("Invalid spl-token account {0:?}")]
    InvalidSplTokenAccount(Pubkey),

    #[error("Missing private key for {0:?}")]
    MissingPrivateKey(Pubkey),

    #[error("Incorrect token decimals")]
    IncorrectTokenDecimals,

    #[error("Treasury pool seed mismatch")]
    TreasuryPoolSeedMismatch,

    #[error("Incorrect program authority")]
    IncorrectProgramAuthority,
}

fn read_keys_dir(keys_dir: &str) -> Result<HashMap<Pubkey,Keypair>,NeonCliError> {
    let mut keys = HashMap::new();
    for file in Path::new(keys_dir).read_dir()?
    {
        let path = file?.path();
        match read_keypair_file(path.clone()) {
            Ok(keypair) => {keys.insert(keypair.pubkey(), keypair);},
            Err(err) => warn!("Skip '{}' due to {}", path.display(), err),
        };
    }
    Ok(keys)
}

#[allow(clippy::too_many_lines)]
pub fn execute(
    config: &Config,
    send_trx: bool,
    force: bool,
    keys_dir: Option<&str>,
    file: Option<&str>,
) -> Result<(), NeonCliError> {

    info!("Signer: {}, send_trx: {}, force: {}", config.signer.pubkey(), send_trx, force);
    let fee_payer = config.fee_payer.as_ref().map_or_else(|| config.signer.as_ref(), |v| v);
    let executor = TransactionExecutor::new(&config.rpc_client, fee_payer, send_trx);
    let keys = keys_dir.map_or(Ok(HashMap::new()), read_keys_dir)?;

    let program_data_address = Pubkey::find_program_address(
        &[&config.evm_loader.to_bytes()],
        &bpf_loader_upgradeable::id()
    ).0;
    let (program_upgrade_authority, program_data) = read_program_data_from_account(config, &config.evm_loader)?;
    let data = file.map_or(Ok(program_data), read_program_data)?;
    let program_parameters = Parameters::new(read_elf_parameters(config, &data));

    let neon_revision = program_parameters.get::<String>("NEON_REVISION")?;
    if neon_revision != neon_cli_revision!() {
        if force {
            warn!("NeonEVM revision doesn't match CLI revision. This check has been disabled with `--force` flag");
        } else {
            error!("NeonEVM revision doesn't match CLI revision. Use appropriate neon-cli version or add `--force` flag");
            return Err(EnvironmentError::RevisionMismatch(neon_revision, neon_cli_revision!().to_string()).into());
        }
    }

    //====================== Create NEON-token mint ===================================================================
    let neon_token_mint = program_parameters.get::<Pubkey>("NEON_TOKEN_MINT")?;
    let neon_token_mint_decimals = program_parameters.get::<u8>("NEON_TOKEN_MINT_DECIMALS")?;
    executor.check_and_create_object(
        "NEON-token mint",
        executor.get_account_data_pack::<spl_token::state::Mint>(
            &spl_token::id(),
            &neon_token_mint
        ),
        |mint| {
            if mint.decimals != neon_token_mint_decimals {
                error!("Invalid token decimals");
                return Err(EnvironmentError::IncorrectTokenDecimals.into());
            }
            Ok(None)
        },
        || {
            let neon_token_mint_signer = keys.get(&neon_token_mint)
                .ok_or(EnvironmentError::MissingPrivateKey(neon_token_mint))?;
            let data_len = spl_token::state::Mint::LEN;
            let lamports = config.rpc_client.get_minimum_balance_for_rent_exemption(data_len)?;
            let transaction = executor.create_transaction(
                &[
                    system_instruction::create_account(
                        &executor.fee_payer.pubkey(),
                        &neon_token_mint,
                        lamports,
                        data_len as u64,
                        &spl_token::id(),
                    ),
                    spl_token::instruction::initialize_mint2(
                        &spl_token::id(),
                        &neon_token_mint,
                        &config.signer.pubkey(),
                        None,
                        neon_token_mint_decimals,
                    )?,
                ],
                &[neon_token_mint_signer]
            )?;
            Ok(Some(transaction))
        }
    )?;

    executor.checkpoint(config.commitment)?;

    //====================== Create 'Deposit' NEON-token balance ======================================================
    let (deposit_authority, _) = Pubkey::find_program_address(&[b"Deposit"], &config.evm_loader);
    let deposit_address = get_associated_token_address(
        &deposit_authority,
        &neon_token_mint,
    );
    executor.check_and_create_object(
        "NEON Deposit balance",
        executor.get_account_data_pack::<spl_token::state::Account>(
            &spl_token::id(),
            &deposit_address),
        |account| {
            if account.mint != neon_token_mint || account.owner != deposit_authority {
                Err(EnvironmentError::InvalidSplTokenAccount(deposit_address).into())
            } else {
                Ok(None)
            }
        },
        || {
            let transaction = executor.create_transaction_with_payer_only(
                &[
                    spl_associated_token_account::instruction::create_associated_token_account(
                        &executor.fee_payer.pubkey(), 
                        &deposit_authority,
                        &neon_token_mint,
                        &spl_token::id(),
                    )
                ]
            )?;
            Ok(Some(transaction))
        },
    )?;

    //====================== Create main treasury balance =============================================================
    let treasury_pool_seed = program_parameters.get::<String>("NEON_POOL_SEED")?;
    if treasury_pool_seed != TREASURY_POOL_SEED {
        error!("Treasury pool seed mismatch {} != {}", treasury_pool_seed, TREASURY_POOL_SEED);
        return Err(EnvironmentError::TreasuryPoolSeedMismatch.into());
    }
    let main_balance_address = MainTreasury::address(&config.evm_loader).0;
    executor.check_and_create_object("Main treasury pool",
        executor.get_account(&main_balance_address),
        |_| Ok(None),
        || {
            if program_upgrade_authority != Some(config.signer.pubkey()) {
                return Err(EnvironmentError::IncorrectProgramAuthority.into());
            }
            let transaction = executor.create_transaction(
                &[
                    Instruction::new_with_bincode(
                        config.evm_loader,
                        &(0x29_u8),   // evm_loader::instruction::EvmInstruction::CreateMainTreasury
                        vec![
                            AccountMeta::new(main_balance_address, false),
                            AccountMeta::new_readonly(program_data_address, false),
                            AccountMeta::new_readonly(config.signer.pubkey(), true),
                            AccountMeta::new_readonly(spl_token::id(), false),
                            AccountMeta::new_readonly(system_program::id(), false),
                            AccountMeta::new_readonly(native_mint::id(), false),
                            AccountMeta::new(executor.fee_payer.pubkey(), true),
                        ],
                    ),
                ],
                &[config.signer.as_ref()],
            )?;
            Ok(Some(transaction))
        },
    )?;

    //====================== Create auxiliary treasury balances =======================================================
    let treasury_pool_count = program_parameters.get::<u32>("NEON_POOL_COUNT")?;
    for i in 0..treasury_pool_count {
        let minimum_balance = config.rpc_client.get_minimum_balance_for_rent_exemption(0)?;
        let aux_balance_address = Treasury::address(&config.evm_loader, i).0;
        executor.check_and_create_object(&format!("Aux treasury pool {}", i),
            executor.get_account(&aux_balance_address),
            |v| {
                if v.lamports < minimum_balance {
                    let transaction = executor.create_transaction_with_payer_only(
                        &[
                            system_instruction::transfer(
                                &executor.fee_payer.pubkey(),
                                &aux_balance_address,
                                minimum_balance-v.lamports
                            )
                        ]
                    )?;
                    Ok(Some(transaction))
                } else {
                    Ok(None)
                }
            },
            || {
                let transaction = executor.create_transaction_with_payer_only(
                    &[
                        system_instruction::transfer(
                            &executor.fee_payer.pubkey(),
                            &aux_balance_address,
                            minimum_balance
                        ),
                    ]
                )?;
                Ok(Some(transaction))
            }
        )?;
    };

    executor.checkpoint(config.rpc_client.commitment())?;

    let stats = executor.stats.borrow();
    info!("Stats: {:?}", stats);
    if stats.total_objects == stats.corrected_objects {
        Ok(())
    } else if stats.invalid_objects == 0 {
        if send_trx {
            Ok(())
        } else {
            // Some object required modifing
            Err(NeonCliError::IncompleteEnvironment)
        }
    } else {
        // Some error in objects or on applying transactions
        Err(NeonCliError::WrongEnvironment)
    }
}