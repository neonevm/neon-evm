use std::collections::BTreeMap;

use ethnum::U256;
use serde::Serialize;
use serde_json::Value;

use crate::evm::opcode_table::OPCODES;
use crate::evm::tracing::TraceConfig;
use crate::evm::tracing::{EmulationResult, Event, EventListener};
use crate::types::hexbytes::HexBytes;

/// `StructLoggerResult` groups all structured logs emitted by the EVM
/// while replaying a transaction in debug mode as well as transaction
/// execution status, the amount of gas used and the return value
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StructLoggerResult {
    /// Is execution failed or not
    pub failed: bool,
    /// Total used gas but include the refunded gas
    pub gas: u64,
    /// The data after execution or revert reason
    pub return_value: String,
    /// Logs emitted during execution
    pub struct_logs: Vec<StructLog>,
}

/// `StructLog` stores a structured log emitted by the EVM while replaying a
/// transaction in debug mode
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StructLog {
    /// Program counter.
    pc: u64,
    /// Operation name
    op: &'static str,
    /// Amount of used gas
    gas: Option<u64>,
    /// Gas cost for this instruction.
    gas_cost: u64,
    /// Current depth
    depth: usize,
    /// Snapshot of the current memory sate
    #[serde(skip_serializing_if = "Option::is_none")]
    memory: Option<Vec<HexBytes>>, // U256 sized chunks
    /// Snapshot of the current stack sate
    #[serde(skip_serializing_if = "Option::is_none")]
    stack: Option<Vec<U256>>,
    /// Result of the step
    return_data: Option<HexBytes>,
    /// Snapshot of the current storage
    #[serde(skip_serializing_if = "Option::is_none")]
    storage: Option<BTreeMap<U256, U256>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl StructLog {
    #[must_use]
    pub fn new(
        opcode: u8,
        pc: u64,
        gas_cost: u64,
        depth: usize,
        memory: Option<Vec<HexBytes>>,
        stack: Option<Vec<U256>>,
        storage: Option<BTreeMap<U256, U256>>,
    ) -> Self {
        let op = OPCODES[opcode as usize];
        Self {
            pc,
            op,
            gas: None,
            gas_cost,
            depth,
            memory,
            stack,
            return_data: None,
            storage,
            error: None,
        }
    }
}

#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
struct Config {
    enable_memory: bool,
    disable_storage: bool,
    disable_stack: bool,
    enable_return_data: bool,
}

impl From<&TraceConfig> for Config {
    fn from(trace_config: &TraceConfig) -> Self {
        Self {
            enable_memory: trace_config.enable_memory,
            disable_storage: trace_config.disable_storage,
            disable_stack: trace_config.disable_stack,
            enable_return_data: trace_config.enable_return_data,
        }
    }
}

#[derive(Debug)]
pub struct StructLogger {
    config: Config,
    logs: Vec<StructLog>,
    depth: usize,
    storage_access: Option<(U256, U256)>,
}

impl StructLogger {
    #[must_use]
    pub fn new(trace_config: &TraceConfig) -> Self {
        StructLogger {
            config: trace_config.into(),
            logs: vec![],
            depth: 0,
            storage_access: None,
        }
    }
}

impl EventListener for StructLogger {
    fn event(&mut self, event: Event) {
        match event {
            Event::BeginVM { .. } => {
                self.depth += 1;
            }
            Event::EndVM { .. } => {
                self.depth -= 1;
            }
            Event::BeginStep {
                opcode,
                pc,
                stack,
                memory,
            } => {
                let stack = if self.config.disable_stack {
                    None
                } else {
                    Some(
                        stack
                            .iter()
                            .map(|entry| U256::from_be_bytes(*entry))
                            .collect(),
                    )
                };

                let memory = if !self.config.enable_memory || memory.is_empty() {
                    None
                } else {
                    Some(
                        memory
                            .chunks(32)
                            .map(|slice| slice.to_vec().into())
                            .collect(),
                    )
                };

                let storage = if self.config.disable_storage {
                    None
                } else {
                    self.logs
                        .last()
                        .and_then(|log| log.storage.clone())
                        .or(None)
                };

                let log = StructLog::new(opcode, pc as u64, 0, self.depth, memory, stack, storage);
                self.logs.push(log);
            }
            Event::EndStep {
                gas_used,
                return_data,
            } => {
                let last = self
                    .logs
                    .last_mut()
                    .expect("`EndStep` event before `BeginStep`");
                last.gas = Some(gas_used);
                if !self.config.disable_storage {
                    if let Some((index, value)) = self.storage_access.take() {
                        last.storage
                            .get_or_insert_with(Default::default)
                            .insert(index, value);
                    };
                }
                if self.config.enable_return_data {
                    last.return_data = return_data.map(Into::into);
                }
            }
            Event::StorageAccess { index, value } if !self.config.disable_storage => {
                self.storage_access = Some((index, U256::from_be_bytes(value)));
            }
            _ => (),
        };
    }

    fn into_traces(self: Box<Self>, emulation_result: EmulationResult) -> Value {
        let result = StructLoggerResult {
            failed: !emulation_result
                .exit_status
                .is_succeed()
                .expect("Emulation is not completed"),
            gas: emulation_result.used_gas,
            return_value: hex::encode(
                emulation_result
                    .exit_status
                    .into_result()
                    .unwrap_or_default(),
            ),
            struct_logs: self.logs,
        };

        serde_json::to_value(result).expect("Conversion error")
    }
}
