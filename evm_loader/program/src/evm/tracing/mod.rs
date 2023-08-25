use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::account::EthereumAccount;
use crate::executor::Action;
use crate::types::hexbytes::HexBytes;
use crate::types::Address;
use ethnum::U256;
use serde_json::Value;

use super::{Context, ExitStatus};

pub mod tracers;

#[derive(Debug, Clone)]
pub struct EmulationResult {
    pub exit_status: ExitStatus,
    pub steps_executed: u64,
    pub used_gas: u64,
    pub actions: Vec<Action>,
}

pub trait EventListener: Send + Sync + Debug {
    fn event(&mut self, event: Event);
    fn into_traces(self: Box<Self>, emulation_result: EmulationResult) -> Value;
}

pub type TracerType = Arc<RwLock<Box<dyn EventListener>>>;
pub type TracerTypeOpt = Option<TracerType>;

/// Trace event
pub enum Event {
    BeginVM {
        context: Context,
        code: Vec<u8>,
    },
    EndVM {
        status: ExitStatus,
    },
    BeginStep {
        opcode: u8,
        pc: usize,
        stack: Vec<[u8; 32]>,
        memory: Vec<u8>,
    },
    EndStep {
        gas_used: u64,
        return_data: Option<Vec<u8>>,
    },
    StackPush {
        value: [u8; 32],
    },
    MemorySet {
        offset: usize,
        data: Vec<u8>,
    },
    StorageSet {
        index: U256,
        value: [u8; 32],
    },
    StorageAccess {
        index: U256,
        value: [u8; 32],
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockOverrides {
    pub number: Option<u64>,
    #[allow(unused)]
    pub difficulty: Option<U256>, // NOT SUPPORTED by Neon EVM
    pub time: Option<i64>,
    #[allow(unused)]
    pub gas_limit: Option<u64>, // NOT SUPPORTED BY Neon EVM
    #[allow(unused)]
    pub coinbase: Option<Address>, // NOT SUPPORTED BY Neon EVM
    #[allow(unused)]
    pub random: Option<U256>, // NOT SUPPORTED BY Neon EVM
    #[allow(unused)]
    pub base_fee: Option<U256>, // NOT SUPPORTED BY Neon EVM
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverride {
    pub nonce: Option<u64>,
    pub code: Option<HexBytes>,
    pub balance: Option<U256>,
    pub state: Option<HashMap<U256, U256>>,
    pub state_diff: Option<HashMap<U256, U256>>,
}

impl AccountOverride {
    pub fn apply(&self, ether_account: &mut EthereumAccount) {
        if let Some(nonce) = self.nonce {
            ether_account.trx_count = nonce;
        }
        if let Some(balance) = self.balance {
            ether_account.balance = balance;
        }
        #[allow(clippy::cast_possible_truncation)]
        if let Some(code) = &self.code {
            ether_account.code_size = code.len() as u32;
        }
    }
}

pub type AccountOverrides = HashMap<Address, AccountOverride>;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions, clippy::struct_excessive_bools)]
pub struct TraceConfig {
    #[serde(default)]
    pub enable_memory: bool,
    #[serde(default)]
    pub disable_storage: bool,
    #[serde(default)]
    pub disable_stack: bool,
    #[serde(default)]
    pub enable_return_data: bool,
    pub tracer: Option<String>,
    pub timeout: Option<String>,
    pub tracer_config: Value,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct TraceCallConfig {
    #[serde(flatten)]
    pub trace_config: TraceConfig,
    pub block_overrides: Option<BlockOverrides>,
    pub state_overrides: Option<AccountOverrides>,
}

impl From<TraceConfig> for TraceCallConfig {
    fn from(trace_config: TraceConfig) -> Self {
        Self {
            trace_config,
            ..Self::default()
        }
    }
}
