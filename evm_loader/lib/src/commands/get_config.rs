#![allow(clippy::future_not_send)]

use async_trait::async_trait;
use base64::Engine;
use enum_dispatch::enum_dispatch;
use std::collections::BTreeMap;
use tokio::sync::OnceCell;

use serde::{Deserialize, Serialize};
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, transaction::Transaction};

use crate::solana_simulator::SolanaSimulator;
use crate::NeonResult;

use crate::rpc::{CallDbClient, CloneRpcClient};
use serde_with::{serde_as, DisplayFromStr};
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::signature::Signer;

#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct GetConfigResponse {
    pub version: String,
    pub revision: String,
    pub status: Status,
    pub environment: String,
    pub chains: Vec<ChainInfo>,
    pub config: BTreeMap<String, String>,
}

#[allow(clippy::large_enum_variant)]
pub enum ConfigSimulator<'r> {
    CloneRpcClient {
        program_id: Pubkey,
        rpc: &'r CloneRpcClient,
    },
    ProgramTestContext {
        program_id: Pubkey,
        simulator: SolanaSimulator,
    },
}

#[async_trait(?Send)]
#[enum_dispatch]
pub trait BuildConfigSimulator {
    fn use_cache(&self) -> bool;
    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator>;
}

#[async_trait(?Send)]
impl BuildConfigSimulator for CloneRpcClient {
    fn use_cache(&self) -> bool {
        true
    }

    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator> {
        Ok(ConfigSimulator::CloneRpcClient {
            program_id,
            rpc: self,
        })
    }
}

#[async_trait(?Send)]
impl BuildConfigSimulator for CallDbClient {
    fn use_cache(&self) -> bool {
        false
    }

    async fn build_config_simulator(&self, program_id: Pubkey) -> NeonResult<ConfigSimulator> {
        let mut simulator = SolanaSimulator::new_without_sync(self).await?;
        simulator.sync_accounts(self, &[program_id]).await?;

        Ok(ConfigSimulator::ProgramTestContext {
            program_id,
            simulator,
        })
    }
}

#[async_trait(?Send)]
trait ConfigInstructionSimulator {
    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>>;
}

#[async_trait(?Send)]
impl ConfigInstructionSimulator for &CloneRpcClient {
    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>> {
        let tx = Transaction::new_with_payer(&[instruction], Some(&self.key_for_config));

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
impl ConfigInstructionSimulator for SolanaSimulator {
    async fn simulate_solana_instruction(
        &mut self,
        instruction: Instruction,
    ) -> NeonResult<Vec<String>> {
        let payer_pubkey = self.payer().pubkey();

        let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer_pubkey));
        transaction.message.recent_blockhash = self.blockhash();

        let r = self.simulate_legacy_transaction(transaction)?;
        if let Err(e) = r.result {
            return Err(e.into());
        }

        Ok(r.logs)
    }
}

impl ConfigSimulator<'_> {
    const fn program_id(&self) -> Pubkey {
        match self {
            ConfigSimulator::CloneRpcClient { program_id, .. }
            | ConfigSimulator::ProgramTestContext { program_id, .. } => *program_id,
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
            ConfigSimulator::CloneRpcClient { rpc, .. } => {
                rpc.simulate_solana_instruction(instruction).await
            }
            ConfigSimulator::ProgramTestContext { simulator, .. } => {
                simulator.simulate_solana_instruction(instruction).await
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
        match return_data.first() {
            Some(0) => Ok(Status::Emergency),
            Some(1) => Ok(Status::Ok),
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

static CHAINS_CACHE: OnceCell<Vec<ChainInfo>> = OnceCell::const_new();

pub async fn read_chains(
    rpc: &impl BuildConfigSimulator,
    program_id: Pubkey,
) -> NeonResult<Vec<ChainInfo>> {
    if rpc.use_cache() && CHAINS_CACHE.initialized() {
        return Ok(CHAINS_CACHE.get().unwrap().clone());
    }

    let mut simulator = rpc.build_config_simulator(program_id).await?;
    let chains = simulator.get_chains().await?;

    if rpc.use_cache() {
        CHAINS_CACHE.set(chains.clone()).unwrap();
    }

    Ok(chains)
}

pub async fn read_legacy_chain_id(
    rpc: &impl BuildConfigSimulator,
    program_id: Pubkey,
) -> NeonResult<u64> {
    for chain in read_chains(rpc, program_id).await? {
        if chain.name == "neon" {
            return Ok(chain.id);
        }
    }

    unreachable!()
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
            format!("{pubkey}"),
            "BPFLoader2111111111111111111111111111111111"
        );
    }
}
