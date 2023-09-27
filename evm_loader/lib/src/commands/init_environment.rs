use std::rc::Rc;

use serde::{Deserialize, Serialize};

use crate::{context::Context, NeonResult};

use {
    crate::{
        commands::{
            get_neon_elf::{
                read_elf_parameters, read_program_data, read_program_data_from_account,
            },
            transaction_executor::TransactionExecutor,
        },
        errors::NeonError,
        Config,
    },
    evm_loader::{
        account::{MainTreasury, Treasury},
        config::TREASURY_POOL_SEED,
    },
    log::{error, info, warn},
    solana_sdk::{
        bpf_loader_upgradeable,
        instruction::{AccountMeta, Instruction},
        program_pack::Pack,
        pubkey::Pubkey,
        signer::keypair::{read_keypair_file, Keypair},
        signer::Signer,
        system_instruction, system_program,
    },
    spl_associated_token_account::get_associated_token_address,
    spl_token::{self, native_mint},
    std::collections::HashMap,
    std::path::Path,
    thiserror::Error,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct InitEnvironmentReturn {
    pub transactions: Vec<String>,
}

struct Parameters {
    params: HashMap<String, String>,
}

impl Parameters {
    pub fn new(params: HashMap<String, String>) -> Self {
        Self { params }
    }

    pub fn get<T: std::str::FromStr>(&self, name: &str) -> Result<T, NeonError> {
        self.params
            .get(name)
            .ok_or_else(|| EnvironmentError::MissingProgramParameter(name.to_string()))?
            .parse::<T>()
            .map_err(|_| EnvironmentError::InvalidProgramParameter(name.to_string()).into())
    }
}

#[derive(Debug, Error)]
pub enum EnvironmentError {
    #[error("NeonEVM and CLI revisions mismatch {0:?} != {1:?}")]
    RevisionMismatch(String, String),

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

fn read_keys_dir(keys_dir: &str) -> Result<HashMap<Pubkey, Keypair>, NeonError> {
    let mut keys = HashMap::new();
    for file in Path::new(keys_dir).read_dir()? {
        let path = file?.path();
        match read_keypair_file(path.clone()) {
            Ok(keypair) => {
                keys.insert(keypair.pubkey(), keypair);
            }
            Err(err) => warn!("Skip '{}' due to {}", path.display(), err),
        };
    }
    Ok(keys)
}

#[allow(clippy::too_many_lines)]
pub async fn execute(
    config: &Config,
    context: &Context<'_>,
    send_trx: bool,
    force: bool,
    keys_dir: Option<&str>,
    file: Option<&str>,
) -> NeonResult<InitEnvironmentReturn> {
    let signer = context.signer()?;
    info!(
        "Signer: {}, send_trx: {}, force: {}",
        signer.pubkey(),
        send_trx,
        force
    );
    let second_signer: &dyn Signer = &*context.signer()?;
    let fee_payer: &dyn Signer = match config.fee_payer.as_ref() {
        Some(fee_payer) => fee_payer,
        None => second_signer,
    };
    let executor = Rc::new(TransactionExecutor::new(
        context.rpc_client,
        fee_payer,
        send_trx,
    ));
    let keys = keys_dir.map_or(Ok(HashMap::new()), read_keys_dir)?;

    let program_data_address = Pubkey::find_program_address(
        &[&config.evm_loader.to_bytes()],
        &bpf_loader_upgradeable::id(),
    )
    .0;
    let (program_upgrade_authority, program_data) =
        read_program_data_from_account(config, context, &config.evm_loader).await?;
    let data = file.map_or(Ok(program_data), read_program_data)?;
    let program_parameters = Parameters::new(read_elf_parameters(config, &data));

    let neon_revision = program_parameters.get::<String>("NEON_REVISION")?;
    let build_neon_revision =
        build_info::format!("{}", $.version_control.unwrap().git().unwrap().commit_id);
    if neon_revision != build_neon_revision {
        if force {
            warn!("NeonEVM revision doesn't match CLI revision. This check has been disabled with `--force` flag");
        } else {
            error!("NeonEVM revision doesn't match CLI revision. Use appropriate neon-cli version or add `--force` flag");
            return Err(EnvironmentError::RevisionMismatch(
                neon_revision,
                build_neon_revision.to_string(),
            )
            .into());
        }
    }

    //====================== Create NEON-token mint ===================================================================
    let executor_clone = executor.clone();
    let second_signer = context.signer()?;
    let create_token = move |mint: Pubkey, decimals: u8| async move {
        let mint_signer = keys
            .get(&mint)
            .ok_or(EnvironmentError::MissingPrivateKey(mint))?;
        let data_len = spl_token::state::Mint::LEN;
        let lamports = context
            .rpc_client
            .get_minimum_balance_for_rent_exemption(data_len)
            .await?;
        let parameters = &[
            system_instruction::create_account(
                &executor_clone.fee_payer.pubkey(),
                &mint,
                lamports,
                data_len as u64,
                &spl_token::id(),
            ),
            spl_token::instruction::initialize_mint2(
                &spl_token::id(),
                &mint,
                &second_signer.pubkey(),
                None,
                decimals,
            )?,
        ];
        let transaction = executor_clone
            .create_transaction(parameters, &[mint_signer])
            .await?;
        Ok(Some(transaction))
    };

    let neon_token_mint = program_parameters.get::<Pubkey>("NEON_TOKEN_MINT")?;
    let neon_token_mint_decimals = program_parameters.get::<u8>("NEON_TOKEN_MINT_DECIMALS")?;
    executor
        .check_and_create_object(
            "NEON-token mint",
            executor
                .get_account_data_pack::<spl_token::state::Mint>(&spl_token::id(), &neon_token_mint)
                .await,
            |mint| async move {
                if mint.decimals != neon_token_mint_decimals {
                    error!("Invalid token decimals");
                    return Err(EnvironmentError::IncorrectTokenDecimals.into());
                }
                Ok(None)
            },
            || create_token(neon_token_mint, neon_token_mint_decimals),
        )
        .await?;

    executor.checkpoint(config.commitment).await?;

    //====================== Create 'Deposit' NEON-token balance ======================================================
    let (deposit_authority, _) = Pubkey::find_program_address(&[b"Deposit"], &config.evm_loader);
    let deposit_address = get_associated_token_address(&deposit_authority, &neon_token_mint);
    executor
        .check_and_create_object(
            "NEON Deposit balance",
            executor
                .get_account_data_pack::<spl_token::state::Account>(
                    &spl_token::id(),
                    &deposit_address,
                )
                .await,
            |account| async move {
                if account.mint != neon_token_mint || account.owner != deposit_authority {
                    Err(EnvironmentError::InvalidSplTokenAccount(deposit_address).into())
                } else {
                    Ok(None)
                }
            },
            || async {
                let transaction = executor
                    .create_transaction_with_payer_only(&[
                        spl_associated_token_account::instruction::create_associated_token_account(
                            &executor.fee_payer.pubkey(),
                            &deposit_authority,
                            &neon_token_mint,
                            &spl_token::id(),
                        ),
                    ])
                    .await?;
                Ok(Some(transaction))
            },
        )
        .await?;

    //====================== Create main treasury balance =============================================================
    let treasury_pool_seed = program_parameters.get::<String>("NEON_POOL_SEED")?;
    if treasury_pool_seed != TREASURY_POOL_SEED {
        error!(
            "Treasury pool seed mismatch {} != {}",
            treasury_pool_seed, TREASURY_POOL_SEED
        );
        return Err(EnvironmentError::TreasuryPoolSeedMismatch.into());
    }
    let main_balance_address = MainTreasury::address(&config.evm_loader).0;
    executor
        .check_and_create_object(
            "Main treasury pool",
            executor.get_account(&main_balance_address).await,
            |_| async move { Ok(None) },
            || async {
                if program_upgrade_authority != Some(signer.pubkey()) {
                    return Err(EnvironmentError::IncorrectProgramAuthority.into());
                }
                let transaction = executor
                    .create_transaction(
                        &[Instruction::new_with_bincode(
                            config.evm_loader,
                            &(0x29_u8), // evm_loader::instruction::EvmInstruction::CreateMainTreasury
                            vec![
                                AccountMeta::new(main_balance_address, false),
                                AccountMeta::new_readonly(program_data_address, false),
                                AccountMeta::new_readonly(signer.pubkey(), true),
                                AccountMeta::new_readonly(spl_token::id(), false),
                                AccountMeta::new_readonly(system_program::id(), false),
                                AccountMeta::new_readonly(native_mint::id(), false),
                                AccountMeta::new(executor.fee_payer.pubkey(), true),
                            ],
                        )],
                        &[&*signer],
                    )
                    .await?;
                Ok(Some(transaction))
            },
        )
        .await?;

    //====================== Create auxiliary treasury balances =======================================================
    let treasury_pool_count = program_parameters.get::<u32>("NEON_POOL_COUNT")?;
    for i in 0..treasury_pool_count {
        let minimum_balance = context
            .rpc_client
            .get_minimum_balance_for_rent_exemption(0)
            .await?;
        let aux_balance_address = Treasury::address(&config.evm_loader, i).0;
        let executor_clone = executor.clone();
        executor
            .check_and_create_object(
                &format!("Aux treasury pool {i}"),
                executor.get_account(&aux_balance_address).await,
                move |v| async move {
                    if v.lamports < minimum_balance {
                        let transaction = executor_clone
                            .create_transaction_with_payer_only(&[system_instruction::transfer(
                                &executor_clone.fee_payer.pubkey(),
                                &aux_balance_address,
                                minimum_balance - v.lamports,
                            )])
                            .await?;
                        Ok(Some(transaction))
                    } else {
                        Ok(None)
                    }
                },
                || async {
                    let transaction = executor
                        .create_transaction_with_payer_only(&[system_instruction::transfer(
                            &executor.fee_payer.pubkey(),
                            &aux_balance_address,
                            minimum_balance,
                        )])
                        .await?;
                    Ok(Some(transaction))
                },
            )
            .await?;
    }

    executor.checkpoint(context.rpc_client.commitment()).await?;

    {
        let stats = executor.stats.borrow();
        info!("Stats: {:?}", stats);
    }

    let signatures = executor
        .signatures
        .borrow()
        .iter()
        .map(|s| bs58::encode(s).into_string())
        .collect::<Vec<String>>();

    let result = InitEnvironmentReturn {
        transactions: signatures,
    };

    let stats = executor.stats.borrow();

    if stats.total_objects == stats.corrected_objects {
        Ok(result)
    } else if stats.invalid_objects == 0 {
        if send_trx {
            Ok(result)
        } else {
            // Some object required modifing
            Err(NeonError::IncompleteEnvironment)
        }
    } else {
        // Some error in objects or on applying transactions
        Err(NeonError::WrongEnvironment)
    }
}
