use async_trait::async_trait;
use base64::Engine;
use enum_dispatch::enum_dispatch;
use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::{Account, AccountSharedData},
    account_utils::StateMut,
    bpf_loader, bpf_loader_deprecated,
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    instruction::Instruction,
    pubkey::Pubkey,
    rent::Rent,
    signer::Signer,
    transaction::Transaction,
};

use crate::{rpc::Rpc, NeonError, NeonResult};

use crate::rpc::{CallDbClient, CloneRpcClient};
use serde_with::{serde_as, DisplayFromStr};
use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_config::{RpcLargestAccountsConfig, RpcSimulateTransactionConfig};
use tokio::sync::{Mutex, MutexGuard, OnceCell};

#[derive(Debug, Serialize)]
pub enum Status {
    Ok,
    Emergency,
    Unknown,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub id: u64,
    pub name: String,
    #[serde_as(as = "DisplayFromStr")]
    pub token: Pubkey,
}

#[serde_as]
#[derive(Debug, Serialize)]
pub struct GetConfigResponse {
    pub version: String,
    pub revision: String,
    pub status: Status,
    pub environment: String,
    pub chains: Vec<ChainInfo>,
    pub config: BTreeMap<String, String>,
}

impl CallDbClient {
    async fn read_program_data_from_account(&self, program_id: Pubkey) -> NeonResult<Vec<u8>> {
        let Some(account) = self.get_account(&program_id).await?.value else {
            return Err(NeonError::AccountNotFound(program_id));
        };

        if account.owner == bpf_loader::id() || account.owner == bpf_loader_deprecated::id() {
            return Ok(account.data);
        }

        if account.owner != bpf_loader_upgradeable::id() {
            return Err(NeonError::AccountIsNotBpf(program_id));
        }

        if let Ok(UpgradeableLoaderState::Program {
            programdata_address: address,
        }) = account.state()
        {
            let Some(programdata_account) = self.get_account(&address).await?.value else {
                return Err(NeonError::AssociatedPdaNotFound(address, program_id));
            };

            let offset = UpgradeableLoaderState::size_of_programdata_metadata();
            let program_data = &programdata_account.data[offset..];

            Ok(program_data.to_vec())
        } else {
            Err(NeonError::AccountIsNotUpgradeable(program_id))
        }
    }
}

async fn program_test_context() -> MutexGuard<'static, ProgramTestContext> {
    static PROGRAM_TEST_CONTEXT: OnceCell<Mutex<ProgramTestContext>> = OnceCell::const_new();

    async fn init_program_test_context() -> Mutex<ProgramTestContext> {
        Mutex::new(ProgramTest::default().start_with_context().await)
    }

    PROGRAM_TEST_CONTEXT
        .get_or_init(init_program_test_context)
        .await
        .lock()
        .await
}

fn set_program_account(
    program_test_context: &mut ProgramTestContext,
    program_id: Pubkey,
    program_data: Vec<u8>,
) {
    program_test_context.set_account(
        &program_id,
        &AccountSharedData::from(Account {
            lamports: Rent::default().minimum_balance(program_data.len()).max(1),
            data: program_data,
            owner: bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        }),
    );
}

pub enum ConfigSimulator<'r> {
    CloneRpcClient(Pubkey, &'r CloneRpcClient),
    ProgramTestContext(Pubkey, MutexGuard<'static, ProgramTestContext>),
}

#[async_trait(?Send)]
#[enum_dispatch]
pub trait BuildConfigSimulator {
    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator>;
}

#[async_trait(?Send)]
impl BuildConfigSimulator for CloneRpcClient {
    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator> {
        Ok(ConfigSimulator::CloneRpcClient(program_id, self))
    }
}

#[async_trait(?Send)]
impl BuildConfigSimulator for CallDbClient {
    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator> {
        let program_data = self.read_program_data_from_account(program_id).await?;

        let mut program_test_context = program_test_context().await;

        set_program_account(&mut program_test_context, program_id, program_data);
        program_test_context.get_new_latest_blockhash().await?;

        Ok(ConfigSimulator::ProgramTestContext(
            program_id,
            program_test_context,
        ))
    }
}

impl CloneRpcClient {
    async fn simulate_solana_instruction(
        &self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>> {
        let tx =
            Transaction::new_with_payer(&[instruction], Some(&self.get_account_with_sol().await?));

        let result = self
            .simulate_transaction_with_config(
                &tx,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    ..RpcSimulateTransactionConfig::default()
                },
            )
            .await?
            .value;

        if let Some(e) = result.err {
            return Err(e.into());
        }
        Ok(result.logs.unwrap())
    }
}

#[async_trait(?Send)]
trait SolanaInstructionSimulator {
    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>>;
}

#[async_trait(?Send)]
impl SolanaInstructionSimulator for ProgramTestContext {
    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>> {
        let tx = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer],
            self.last_blockhash,
        );

        // TODO: Fix failure to simulate transaction
        // it can come from old NeonEVM program without chain_id support for old tx, when it should return default chain_id info
        let result = self
            .banks_client
            .simulate_transaction(tx)
            .await
            .map_err(|e| NeonError::from(Box::new(e)))?;

        result.result.unwrap()?;

        Ok(result.simulation_details.unwrap().logs)
    }
}

impl ConfigSimulator<'_> {
    fn program_id(&self) -> Pubkey {
        match self {
            ConfigSimulator::CloneRpcClient(program_id, _) => *program_id,
            ConfigSimulator::ProgramTestContext(program_id, _) => *program_id,
        }
    }

    async fn simulate_evm_instruction(
        &mut self,
        evm_instruction: u8,
        data: &[u8],
    ) -> NeonResult<Vec<u8>> {
        fn base64_decode(s: &str) -> Vec<u8> {
            base64::engine::general_purpose::STANDARD.decode(s).unwrap()
        }

        let program_id = self.program_id();

        let logs = self
            .simulate_solana_instruction(Instruction::new_with_bytes(
                program_id,
                &[&[evm_instruction], data].concat(),
                vec![],
            ))
            .await?;

        // Program return: 53DfF883gyixYNXnM7s5xhdeyV8mVk9T4i2hGV9vG9io AQAAAAAAAAA=
        let return_data = logs
            .into_iter()
            .find_map(|msg| {
                let prefix = std::format!("Program return: {program_id} ");
                msg.strip_prefix(&prefix).map(base64_decode)
            })
            .unwrap();

        Ok(return_data)
    }

    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>> {
        match self {
            ConfigSimulator::CloneRpcClient(_, clone_rpc_client) => {
                clone_rpc_client
                    .simulate_solana_instruction(instruction)
                    .await
            }
            ConfigSimulator::ProgramTestContext(_, program_test_context) => {
                program_test_context
                    .simulate_solana_instruction(instruction)
                    .await
            }
        }
    }

    async fn get_version(&mut self) -> NeonResult<(String, String)> {
        let return_data = self.simulate_evm_instruction(0xA7, &[]).await?;
        let (version, revision) = bincode::deserialize(&return_data)?;

        Ok((version, revision))
    }

    async fn get_status(&mut self) -> NeonResult<Status> {
        let return_data = self.simulate_evm_instruction(0xA6, &[]).await?;
        match return_data[0] {
            0 => Ok(Status::Emergency),
            1 => Ok(Status::Ok),
            _ => Ok(Status::Unknown),
        }
    }

    async fn get_environment(&mut self) -> NeonResult<String> {
        let return_data = self.simulate_evm_instruction(0xA2, &[]).await?;
        let environment = String::from_utf8(return_data)?;

        Ok(environment)
    }

    async fn get_chains(&mut self) -> NeonResult<Vec<ChainInfo>> {
        let mut result = Vec::new();

        let return_data = self.simulate_evm_instruction(0xA0, &[]).await?;
        let chain_count = return_data.as_slice().try_into()?;
        let chain_count = usize::from_le_bytes(chain_count);

        for i in 0..chain_count {
            let index = i.to_le_bytes();
            let return_data = self.simulate_evm_instruction(0xA1, &index).await?;

            let (id, name, token) = bincode::deserialize(&return_data)?;
            result.push(ChainInfo { id, name, token });
        }

        Ok(result)
    }

    async fn get_properties(&mut self) -> NeonResult<BTreeMap<String, String>> {
        let mut result = BTreeMap::new();

        let return_data = self.simulate_evm_instruction(0xA3, &[]).await?;
        let count = return_data.as_slice().try_into()?;
        let count = usize::from_le_bytes(count);

        for i in 0..count {
            let index = i.to_le_bytes();
            let return_data = self.simulate_evm_instruction(0xA4, &index).await?;

            let (name, value) = bincode::deserialize(&return_data)?;
            result.insert(name, value);
        }

        Ok(result)
    }
}

pub async fn execute(
    rpc: &impl BuildConfigSimulator,
    program_id: Pubkey,
) -> NeonResult<GetConfigResponse> {
    let mut simulator = rpc.build_config_simulator(program_id).await?;

    let (version, revision) = simulator.get_version().await?;

    Ok(GetConfigResponse {
        version,
        revision,
        status: simulator.get_status().await?,
        environment: simulator.get_environment().await?,
        chains: simulator.get_chains().await?,
        config: simulator.get_properties().await?,
    })
}

pub async fn read_chains(
    rpc: &impl BuildConfigSimulator,
    program_id: Pubkey,
) -> NeonResult<Vec<ChainInfo>> {
    let mut simulator = rpc.build_config_simulator(program_id).await?;

    simulator.get_chains().await
}

pub async fn read_default_chain_id(
    rpc: &impl BuildConfigSimulator,
    program_id: Pubkey,
) -> NeonResult<u64> {
    let mut simulator = rpc.build_config_simulator(program_id).await?;

    let chains = simulator.get_chains().await?;
    let default_chain = chains.iter().find(|chain| chain.name == "neon").unwrap();

    Ok(default_chain.id)
}

impl CloneRpcClient {
    async fn get_account_with_sol(&self) -> ClientResult<Pubkey> {
        let r = self
            .get_largest_accounts_with_config(RpcLargestAccountsConfig {
                commitment: Some(self.commitment()),
                filter: None,
            })
            .await?; // TODO https://neonlabs.atlassian.net/browse/NDEV-2462 replace with more efficient RPC call

        Ok(Pubkey::from_str(&r.value[0].address).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bpf_loader_pubkey() {
        let pubkey = Pubkey::from([
            2, 168, 246, 145, 78, 136, 161, 110, 57, 90, 225, 40, 148, 143, 250, 105, 86, 147, 55,
            104, 24, 221, 71, 67, 82, 33, 243, 198, 0, 0, 0, 0,
        ]);
        assert_eq!(
            format!("{}", pubkey),
            "BPFLoader2111111111111111111111111111111111"
        );
    }
}
