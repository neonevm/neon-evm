use std::fmt::{Display, Formatter};

use ethnum::U256;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey};

use evm_loader::evm::tracing::TracerTypeOpt;
use evm_loader::evm::tracing::{AccountOverrides, BlockOverrides};
use evm_loader::{
    account_storage::AccountStorage,
    config::{EVM_STEPS_MIN, PAYMENT_TO_TREASURE},
    evm::{ExitStatus, Machine},
    executor::{Action, ExecutorState},
    gasometer::LAMPORTS_PER_SIGNATURE,
    types::{Address, Transaction},
};

use crate::types::TxParams;
use crate::{
    account_storage::{EmulatorAccountStorage, NeonAccount, SolanaAccount},
    errors::NeonError,
    rpc::Rpc,
    syscall_stubs::Stubs,
    NeonResult,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationResult {
    #[serde(serialize_with = "serde_hex_serialize")]
    #[serde(deserialize_with = "serde_hex_deserialize")]
    pub result: Vec<u8>,
    pub exit_status: String,
    pub steps_executed: u64,
    pub used_gas: u64,
    pub actions: Vec<Action>,
}

impl Display for EmulationResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ exit_status: {}, steps_executed: {}, used_gas: {}, actions: {}, result: {} }}",
            self.exit_status,
            self.steps_executed,
            self.used_gas,
            self.actions.len(),
            hex::encode(&self.result),
        )
    }
}

impl From<evm_loader::evm::tracing::EmulationResult> for EmulationResult {
    fn from(value: evm_loader::evm::tracing::EmulationResult) -> Self {
        Self {
            exit_status: value.exit_status.status().to_string(),
            result: value.exit_status.into_result().unwrap_or_default(),
            steps_executed: value.steps_executed,
            used_gas: value.used_gas,
            actions: value.actions,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationResultWithAccounts {
    pub accounts: Vec<NeonAccount>,
    pub solana_accounts: Vec<SolanaAccount>,
    pub token_accounts: Vec<SolanaAccount>,
    #[serde(flatten)]
    pub emulation_result: EmulationResult,
}

impl Display for EmulationResultWithAccounts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.emulation_result)
    }
}

fn serde_hex_serialize<S>(value: &[u8], s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&hex::encode(value))
}

fn serde_hex_deserialize<'de, D>(d: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringVisitor;
    impl<'de> serde::de::Visitor<'de> for StringVisitor {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            write!(formatter, "a hex-encoded string with even length")
        }

        fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            hex::decode(s).map_err(|_err| {
                serde::de::Error::invalid_value(serde::de::Unexpected::Str(s), &self)
            })
        }
    }

    d.deserialize_string(StringVisitor)
}

#[allow(clippy::too_many_arguments)]
pub async fn execute(
    rpc_client: &dyn Rpc,
    evm_loader: Pubkey,
    tx_params: TxParams,
    token_mint: Pubkey,
    chain_id: u64,
    step_limit: u64,
    commitment: CommitmentConfig,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
    block_overrides: &Option<BlockOverrides>,
    state_overrides: Option<AccountOverrides>,
) -> NeonResult<EmulationResultWithAccounts> {
    let (emulation_result, storage) = emulate_transaction(
        rpc_client,
        evm_loader,
        tx_params,
        token_mint,
        chain_id,
        step_limit,
        commitment,
        accounts,
        solana_accounts,
        block_overrides,
        state_overrides,
        None,
    )
    .await?;
    let accounts = storage.accounts.borrow().values().cloned().collect();
    let solana_accounts = storage.solana_accounts.borrow().values().cloned().collect();

    Ok(EmulationResultWithAccounts {
        accounts,
        solana_accounts,
        token_accounts: vec![],
        emulation_result: emulation_result.into(),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn emulate_transaction<'a>(
    rpc_client: &'a dyn Rpc,
    evm_loader: Pubkey,
    tx_params: TxParams,
    token_mint: Pubkey,
    chain_id: u64,
    step_limit: u64,
    commitment: CommitmentConfig,
    accounts: &[Address],
    solana_accounts: &[Pubkey],
    block_overrides: &Option<BlockOverrides>,
    state_overrides: Option<AccountOverrides>,
    tracer: TracerTypeOpt,
) -> Result<
    (
        evm_loader::evm::tracing::EmulationResult,
        EmulatorAccountStorage<'a>,
    ),
    NeonError,
> {
    setup_syscall_stubs(rpc_client).await?;

    let storage = EmulatorAccountStorage::with_accounts(
        rpc_client,
        evm_loader,
        token_mint,
        chain_id,
        commitment,
        accounts,
        solana_accounts,
        block_overrides,
        state_overrides,
    )
    .await?;

    emulate_trx(tx_params, &storage, chain_id, step_limit, tracer)
        .await
        .map(move |result| (result, storage))
}

pub(crate) async fn emulate_trx<'a>(
    tx_params: TxParams,
    storage: &'a EmulatorAccountStorage<'a>,
    chain_id: u64,
    step_limit: u64,
    tracer: TracerTypeOpt,
) -> Result<evm_loader::evm::tracing::EmulationResult, NeonError> {
    let (exit_status, actions, steps_executed) = {
        let mut backend = ExecutorState::new(storage);
        let trx_payload = if tx_params.access_list.is_some() {
            let access_list = tx_params
                .access_list
                .expect("access_list is present")
                .into_iter()
                .map(|item| {
                    (
                        item.address,
                        item.storage_keys
                            .into_iter()
                            .map(|k| {
                                evm_loader::types::StorageKey::try_from(k)
                                    .expect("key to be correct")
                            })
                            .collect(),
                    )
                })
                .collect();
            evm_loader::types::TransactionPayload::AccessList(evm_loader::types::AccessListTx {
                nonce: match tx_params.nonce {
                    Some(nonce) => nonce,
                    None => storage.nonce(&tx_params.from).await,
                },
                gas_price: U256::ZERO,
                gas_limit: tx_params.gas_limit.unwrap_or(U256::MAX),
                target: tx_params.to,
                value: tx_params.value.unwrap_or_default(),
                call_data: evm_loader::evm::Buffer::from_slice(&tx_params.data.unwrap_or_default()),
                r: U256::default(),
                s: U256::default(),
                chain_id: chain_id.into(),
                recovery_id: u8::default(),
                access_list,
            })
        } else {
            evm_loader::types::TransactionPayload::Legacy(evm_loader::types::LegacyTx {
                nonce: match tx_params.nonce {
                    Some(nonce) => nonce,
                    None => storage.nonce(&tx_params.from).await,
                },
                gas_price: U256::ZERO,
                gas_limit: tx_params.gas_limit.unwrap_or(U256::MAX),
                target: tx_params.to,
                value: tx_params.value.unwrap_or_default(),
                call_data: evm_loader::evm::Buffer::from_slice(&tx_params.data.unwrap_or_default()),
                v: U256::default(),
                r: U256::default(),
                s: U256::default(),
                chain_id: Some(chain_id.into()),
                recovery_id: u8::default(),
            })
        };

        let mut trx = Transaction {
            transaction: trx_payload,
            byte_len: usize::default(),
            hash: <[u8; 32]>::default(),
            signed_hash: <[u8; 32]>::default(),
        };

        let mut evm = Machine::new(&mut trx, tx_params.from, &mut backend, tracer).await?;

        let (result, steps_executed) = evm.execute(step_limit, &mut backend).await?;
        if result == ExitStatus::StepLimit {
            return Err(NeonError::TooManySteps);
        }

        let actions = backend.into_actions();
        (result, actions, steps_executed)
    };

    debug!("Execute done, result={exit_status:?}");
    debug!("{steps_executed} steps executed");

    let accounts_operations = storage.calc_accounts_operations(&actions).await;

    let max_iterations = (steps_executed + (EVM_STEPS_MIN - 1)) / EVM_STEPS_MIN;
    let steps_gas = max_iterations * (LAMPORTS_PER_SIGNATURE + PAYMENT_TO_TREASURE);
    let begin_end_gas = 2 * LAMPORTS_PER_SIGNATURE;
    let actions_gas = storage.apply_actions(&actions).await;
    let accounts_gas = storage.apply_accounts_operations(accounts_operations).await;
    info!("Gas - steps: {steps_gas}, actions: {actions_gas}, accounts: {accounts_gas}");

    Ok(evm_loader::evm::tracing::EmulationResult {
        exit_status,
        steps_executed,
        used_gas: steps_gas + begin_end_gas + actions_gas + accounts_gas,
        actions,
    })
}

pub(crate) async fn setup_syscall_stubs(rpc_client: &dyn Rpc) -> Result<(), NeonError> {
    let syscall_stubs = Stubs::new(rpc_client).await?;
    solana_sdk::program_stubs::set_syscall_stubs(syscall_stubs);

    Ok(())
}
