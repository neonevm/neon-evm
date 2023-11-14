use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;

use crate::types::hexbytes::HexBytes;
use crate::types::Address;
use ethnum::U256;
use serde_json::Value;

use super::{Context, ExitStatus};

pub mod tracers;

pub trait EventListener: Send + Sync + Debug {
    fn event(&mut self, event: Event);
    fn into_traces(self: Box<Self>) -> Value;
}

pub type TracerType = Rc<RefCell<Box<dyn EventListener>>>;
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
    StorageAccess {
        index: U256,
        value: [u8; 32],
    },
}

/// See <https://github.com/ethereum/go-ethereum/blob/master/internal/ethapi/api.go#L993>
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

/// See <https://github.com/ethereum/go-ethereum/blob/master/internal/ethapi/api.go#L942>
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
    #[must_use]
    pub fn storage(&self, index: U256) -> Option<[u8; 32]> {
        match (&self.state, &self.state_diff) {
            (None, None) => None,
            (Some(_), Some(_)) => {
                panic!("Account has both `state` and `stateDiff` overrides")
            }
            (Some(state), None) => return state.get(&index).map(|value| value.to_be_bytes()),
            (None, Some(state_diff)) => state_diff.get(&index).map(|v| v.to_be_bytes()),
        }
    }
}

/// See <https://github.com/ethereum/go-ethereum/blob/master/internal/ethapi/api.go#L951>
pub type AccountOverrides = HashMap<Address, AccountOverride>;

/// See <https://github.com/ethereum/go-ethereum/blob/master/eth/tracers/api.go#L151>
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
    pub tracer_config: Option<Value>,
}

/// See <https://github.com/ethereum/go-ethereum/blob/master/eth/tracers/api.go#L163>
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
