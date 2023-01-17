use super::Bytes;
use evm_loader::{U256, ExitReason};
use lazy_static::lazy_static;
use log::warn;
use std::cmp::min;


#[derive(serde::Serialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some chunk of memory.
pub struct MemoryDiff {
    /// Offset into memory the change begins.
    pub offset: usize,
    /// The changed data.
    pub data: Bytes,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some storage value.
pub struct StorageDiff {
    /// Which key in storage is changed.
    pub location: U256,
    /// What the value has been changed to.
    pub value: U256,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A record of an executed VM operation.
pub struct VMExecutedOperation {
    /// The total gas used.
    pub gas_used: U256,
    /// The stack item placed, if any.
    pub stack_push: Vec<U256>,
    /// If altered, the memory delta.
    pub mem_diff: Option<MemoryDiff>,
    /// The altered storage value, if any.
    pub store_diff: Option<StorageDiff>,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
/// A record of the execution of a single VM operation.
pub struct VMOperation {
    /// The program counter.
    pub pc: usize,
    /// The instruction executed.
    pub instruction: u8,
    /// The gas cost for this instruction.
    pub gas_cost: U256,
    /// Information concerning the execution of the operation.
    pub executed: Option<VMExecutedOperation>,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
/// A record of a full VM trace for a CALL/CREATE.
#[allow(clippy::module_name_repetitions)]
pub struct VMTrace {
    /// The step (i.e. index into operations) at which this trace corresponds.
    pub parent_step: usize,
    /// The code to be executed.
    pub code: Bytes,
    /// The operations executed.
    pub operations: Vec<VMOperation>,
    /// The sub traces for each interior action performed as part of this call/create.
    /// Thre is a 1:1 correspondance between these and a CALL/CREATE/CALLCODE/DELEGATECALL instruction.
    pub subs: Vec<VMTrace>,
}

// OpenEthereum tracer ethcore/src/trace/executive_tracer.rs
struct TraceData {
    mem_written: Option<(usize, usize)>,
    store_written: Option<(U256, U256)>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct FullTraceData {
    pub stack: Vec<U256>,
    pub memory: Vec<u8>,
    pub storage: Option<(U256, U256)>,
}

/// Simple VM tracer. Traces all operations.
pub struct ExecutiveVMTracer {
    data: VMTrace,
    pub depth: usize,
    trace_stack: Vec<TraceData>,
}

impl ExecutiveVMTracer {
    /// Create a new top-level instance.
    #[allow(dead_code)]
    pub fn toplevel() -> Self {
        ExecutiveVMTracer {
            data: VMTrace {
                parent_step: 0,
                code: vec![].into(),
                operations: vec![VMOperation::default()], // prefill with a single entry so that prepare_subtrace can get the parent_step
                subs: vec![],
            },
            depth: 0,
            trace_stack: vec![],
        }
    }

    fn with_trace_in_depth<F: Fn(&mut VMTrace)>(trace: &mut VMTrace, depth: usize, f: F) {
        if depth == 0 {
            f(trace);
        } else {
            Self::with_trace_in_depth(trace.subs.last_mut().expect("self.depth is incremented with prepare_subtrace; a subtrace is always pushed; self.depth cannot be greater than subtrace stack; qed"), depth - 1, f);
        }
    }
}

impl VMTracer for ExecutiveVMTracer {
    type Output = VMTrace;


    fn trace_prepare_execute(
        &mut self,
        pc: usize,
        instruction: u8,
        gas_cost: U256,
        mem_written: Option<(usize, usize)>,
        store_written: Option<(U256, U256)>,
    ) {
        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            trace.operations.push(VMOperation {
                pc: pc,
                instruction: instruction,
                gas_cost: gas_cost,
                executed: None,
            });
        });
        self.trace_stack.push(TraceData {
            mem_written,
            store_written,
        });
    }

    fn trace_failed(&mut self) {
        let _ = self
            .trace_stack
            .pop()
            .expect("pushed in trace_prepare_execute; qed");
    }

    fn trace_executed(&mut self, gas_used: U256, stack_push: &[U256], mem: &[u8]) {
        let TraceData {
            mem_written,
            store_written,
        } = self
            .trace_stack
            .pop()
            .expect("pushed in trace_prepare_execute; qed");
        let mem_diff = mem_written.map(|(o, s)| {
            if o + s > mem.len() {
                warn!(target: "trace", "mem_written is out of bounds");
            }
            (o, &mem[min(mem.len(), o)..min(o + s, mem.len())])
        });
        let store_diff = store_written;
        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            let ex = VMExecutedOperation {
                gas_used: gas_used,
                stack_push: stack_push.to_vec(),
                mem_diff: mem_diff.map(|(s, r)| MemoryDiff {
                    offset: s,
                    data: r.to_vec(),
                }),
                store_diff: store_diff.map(|(l, v)| StorageDiff {
                    location: l,
                    value: v,
                }),
            };
            trace.operations.last_mut().expect("trace_executed is always called after a trace_prepare_execute; trace.operations cannot be empty; qed").executed = Some(ex);
        });
    }

    fn prepare_subtrace(&mut self, code: &[u8]) {
        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            let parent_step = trace.operations.len() - 1; // won't overflow since we must already have pushed an operation in trace_prepare_execute.
            trace.subs.push(VMTrace {
                parent_step,
                code: code.to_vec(),
                operations: vec![],
                subs: vec![],
            });
        });
        self.depth += 1;
    }

    fn done_subtrace(&mut self) {
        self.depth -= 1;
    }

    fn drain(mut self) -> Option<VMTrace> {
        self.data.subs.pop()
    }
}

// ethcore/src/trace/mod.rs
pub trait VMTracer: Send {
    /// Data returned when draining the `VMTracer`.
    type Output;

    /// Trace the preparation to execute a single valid instruction.
    fn trace_prepare_execute(
        &mut self,
        _pc: usize,
        _instruction: u8,
        _gas_cost: U256,
        _mem_written: Option<(usize, usize)>,
        _store_written: Option<(U256, U256)>,
    ) {
    }

    /// Trace the execution failure of a single instruction.
    fn trace_failed(&mut self) {}

    /// Trace the finalised execution of a single valid instruction.
    fn trace_executed(&mut self, _gas_used: U256, _stack_push: &[U256], _mem: &[u8]) {}

    /// Spawn subtracer which will be used to trace deeper levels of execution.
    fn prepare_subtrace(&mut self, _code: &[u8]) {}

    /// Finalize subtracer.
    fn done_subtrace(&mut self) {}

    /// Consumes self and returns the VM trace.
    fn drain(self) -> Option<Self::Output>;
}

/// This trait is used by executive to build traces.
pub trait Tracer: Send {
    /// Data returned when draining the Tracer.
    type Output;

    /// Consumes self and returns all traces.
    fn drain(self) -> Vec<Self::Output>;
}

#[derive(Copy, Clone)]
pub struct InstructionInfo {
    /// Mnemonic name.
    pub name: &'static str,
    /// Number of stack arguments.
    pub args: usize,
    /// Number of returned stack items.
    pub ret: usize,
}

impl InstructionInfo {
    /// Create new instruction info.
    pub fn new(name: &'static str, args: usize, ret: usize) -> Self {
        InstructionInfo {
            name,
            args,
            ret,
        }
    }
}


lazy_static! {
    /// Static instruction table.
    pub static ref INSTRUCTIONS: [Option<InstructionInfo>; 0x100] = {
        use evm_loader::Opcode;
        let mut arr = [None; 0x100];
        arr[Opcode::STOP.as_usize()] = Some(InstructionInfo::new("STOP", 0, 0));
        arr[Opcode::ADD.as_usize()] = Some(InstructionInfo::new("ADD", 2, 1));
        arr[Opcode::SUB.as_usize()] = Some(InstructionInfo::new("SUB", 2, 1));
        arr[Opcode::MUL.as_usize()] = Some(InstructionInfo::new("MUL", 2, 1));
        arr[Opcode::DIV.as_usize()] = Some(InstructionInfo::new("DIV", 2, 1));
        arr[Opcode::SDIV.as_usize()] = Some(InstructionInfo::new("SDIV", 2, 1));
        arr[Opcode::MOD.as_usize()] = Some(InstructionInfo::new("MOD", 2, 1));
        arr[Opcode::SMOD.as_usize()] = Some(InstructionInfo::new("SMOD", 2, 1));
        arr[Opcode::EXP.as_usize()] = Some(InstructionInfo::new("EXP", 2, 1));
        arr[Opcode::NOT.as_usize()] = Some(InstructionInfo::new("NOT", 1, 1));
        arr[Opcode::LT.as_usize()] = Some(InstructionInfo::new("LT", 2, 1));
        arr[Opcode::GT.as_usize()] = Some(InstructionInfo::new("GT", 2, 1));
        arr[Opcode::SLT.as_usize()] = Some(InstructionInfo::new("SLT", 2, 1));
        arr[Opcode::SGT.as_usize()] = Some(InstructionInfo::new("SGT", 2, 1));
        arr[Opcode::EQ.as_usize()] = Some(InstructionInfo::new("EQ", 2, 1));
        arr[Opcode::ISZERO.as_usize()] = Some(InstructionInfo::new("ISZERO", 1, 1));
        arr[Opcode::AND.as_usize()] = Some(InstructionInfo::new("AND", 2, 1));
        arr[Opcode::OR.as_usize()] = Some(InstructionInfo::new("OR", 2, 1));
        arr[Opcode::XOR.as_usize()] = Some(InstructionInfo::new("XOR", 2, 1));
        arr[Opcode::BYTE.as_usize()] = Some(InstructionInfo::new("BYTE", 2, 1));
        arr[Opcode::SHL.as_usize()] = Some(InstructionInfo::new("SHL", 2, 1));
        arr[Opcode::SHR.as_usize()] = Some(InstructionInfo::new("SHR", 2, 1));
        arr[Opcode::SAR.as_usize()] = Some(InstructionInfo::new("SAR", 2, 1));
        arr[Opcode::ADDMOD.as_usize()] = Some(InstructionInfo::new("ADDMOD", 3, 1));
        arr[Opcode::MULMOD.as_usize()] = Some(InstructionInfo::new("MULMOD", 3, 1));
        arr[Opcode::SIGNEXTEND.as_usize()] = Some(InstructionInfo::new("SIGNEXTEND", 2, 1));
        arr[Opcode::RETURNDATASIZE.as_usize()] = Some(InstructionInfo::new("RETURNDATASIZE", 0, 1));
        arr[Opcode::RETURNDATACOPY.as_usize()] = Some(InstructionInfo::new("RETURNDATACOPY", 3, 0));
        arr[Opcode::SHA3.as_usize()] = Some(InstructionInfo::new("SHA3", 2, 1));
        arr[Opcode::ADDRESS.as_usize()] = Some(InstructionInfo::new("ADDRESS", 0, 1));
        arr[Opcode::BALANCE.as_usize()] = Some(InstructionInfo::new("BALANCE", 1, 1));
        arr[Opcode::ORIGIN.as_usize()] = Some(InstructionInfo::new("ORIGIN", 0, 1));
        arr[Opcode::CALLER.as_usize()] = Some(InstructionInfo::new("CALLER", 0, 1));
        arr[Opcode::CALLVALUE.as_usize()] = Some(InstructionInfo::new("CALLVALUE", 0, 1));
        arr[Opcode::CALLDATALOAD.as_usize()] = Some(InstructionInfo::new("CALLDATALOAD", 1, 1));
        arr[Opcode::CALLDATASIZE.as_usize()] = Some(InstructionInfo::new("CALLDATASIZE", 0, 1));
        arr[Opcode::CALLDATACOPY.as_usize()] = Some(InstructionInfo::new("CALLDATACOPY", 3, 0));
        arr[Opcode::EXTCODEHASH.as_usize()] = Some(InstructionInfo::new("EXTCODEHASH", 1, 1));
        arr[Opcode::CODESIZE.as_usize()] = Some(InstructionInfo::new("CODESIZE", 0, 1));
        arr[Opcode::CODECOPY.as_usize()] = Some(InstructionInfo::new("CODECOPY", 3, 0));
        arr[Opcode::GASPRICE.as_usize()] = Some(InstructionInfo::new("GASPRICE", 0, 1));
        arr[Opcode::EXTCODESIZE.as_usize()] = Some(InstructionInfo::new("EXTCODESIZE", 1, 1));
        arr[Opcode::EXTCODECOPY.as_usize()] = Some(InstructionInfo::new("EXTCODECOPY", 4, 0));
        arr[Opcode::BLOCKHASH.as_usize()] = Some(InstructionInfo::new("BLOCKHASH", 1, 1));
        arr[Opcode::COINBASE.as_usize()] = Some(InstructionInfo::new("COINBASE", 0, 1));
        arr[Opcode::TIMESTAMP.as_usize()] = Some(InstructionInfo::new("TIMESTAMP", 0, 1));
        arr[Opcode::NUMBER.as_usize()] = Some(InstructionInfo::new("NUMBER", 0, 1));
        arr[Opcode::DIFFICULTY.as_usize()] = Some(InstructionInfo::new("DIFFICULTY", 0, 1));
        arr[Opcode::GASLIMIT.as_usize()] = Some(InstructionInfo::new("GASLIMIT", 0, 1));
        arr[Opcode::CHAINID.as_usize()] = Some(InstructionInfo::new("CHAINID", 0, 1));
        arr[Opcode::SELFBALANCE.as_usize()] = Some(InstructionInfo::new("SELFBALANCE", 0, 1));
        //arr[Opcode::BASEFEE.as_usize()] = Some(InstructionInfo::new("BASEFEE", 0, 1));
        arr[Opcode::POP.as_usize()] = Some(InstructionInfo::new("POP", 1, 0));
        arr[Opcode::MLOAD.as_usize()] = Some(InstructionInfo::new("MLOAD", 1, 1));
        arr[Opcode::MSTORE.as_usize()] = Some(InstructionInfo::new("MSTORE", 2, 0));
        arr[Opcode::MSTORE8.as_usize()] = Some(InstructionInfo::new("MSTORE8", 2, 0));
        arr[Opcode::SLOAD.as_usize()] = Some(InstructionInfo::new("SLOAD", 1, 1));
        arr[Opcode::SSTORE.as_usize()] = Some(InstructionInfo::new("SSTORE", 2, 0));
        arr[Opcode::JUMP.as_usize()] = Some(InstructionInfo::new("JUMP", 1, 0));
        arr[Opcode::JUMPI.as_usize()] = Some(InstructionInfo::new("JUMPI", 2, 0));
        arr[Opcode::PC.as_usize()] = Some(InstructionInfo::new("PC", 0, 1));
        arr[Opcode::MSIZE.as_usize()] = Some(InstructionInfo::new("MSIZE", 0, 1));
        arr[Opcode::GAS.as_usize()] = Some(InstructionInfo::new("GAS", 0, 1));
        arr[Opcode::JUMPDEST.as_usize()] = Some(InstructionInfo::new("JUMPDEST", 0, 0));
        arr[Opcode::PUSH1.as_usize()] = Some(InstructionInfo::new("PUSH1", 0, 1));
        arr[Opcode::PUSH2.as_usize()] = Some(InstructionInfo::new("PUSH2", 0, 1));
        arr[Opcode::PUSH3.as_usize()] = Some(InstructionInfo::new("PUSH3", 0, 1));
        arr[Opcode::PUSH4.as_usize()] = Some(InstructionInfo::new("PUSH4", 0, 1));
        arr[Opcode::PUSH5.as_usize()] = Some(InstructionInfo::new("PUSH5", 0, 1));
        arr[Opcode::PUSH6.as_usize()] = Some(InstructionInfo::new("PUSH6", 0, 1));
        arr[Opcode::PUSH7.as_usize()] = Some(InstructionInfo::new("PUSH7", 0, 1));
        arr[Opcode::PUSH8.as_usize()] = Some(InstructionInfo::new("PUSH8", 0, 1));
        arr[Opcode::PUSH9.as_usize()] = Some(InstructionInfo::new("PUSH9", 0, 1));
        arr[Opcode::PUSH10.as_usize()] = Some(InstructionInfo::new("PUSH10", 0, 1));
        arr[Opcode::PUSH11.as_usize()] = Some(InstructionInfo::new("PUSH11", 0, 1));
        arr[Opcode::PUSH12.as_usize()] = Some(InstructionInfo::new("PUSH12", 0, 1));
        arr[Opcode::PUSH13.as_usize()] = Some(InstructionInfo::new("PUSH13", 0, 1));
        arr[Opcode::PUSH14.as_usize()] = Some(InstructionInfo::new("PUSH14", 0, 1));
        arr[Opcode::PUSH15.as_usize()] = Some(InstructionInfo::new("PUSH15", 0, 1));
        arr[Opcode::PUSH16.as_usize()] = Some(InstructionInfo::new("PUSH16", 0, 1));
        arr[Opcode::PUSH17.as_usize()] = Some(InstructionInfo::new("PUSH17", 0, 1));
        arr[Opcode::PUSH18.as_usize()] = Some(InstructionInfo::new("PUSH18", 0, 1));
        arr[Opcode::PUSH19.as_usize()] = Some(InstructionInfo::new("PUSH19", 0, 1));
        arr[Opcode::PUSH20.as_usize()] = Some(InstructionInfo::new("PUSH20", 0, 1));
        arr[Opcode::PUSH21.as_usize()] = Some(InstructionInfo::new("PUSH21", 0, 1));
        arr[Opcode::PUSH22.as_usize()] = Some(InstructionInfo::new("PUSH22", 0, 1));
        arr[Opcode::PUSH23.as_usize()] = Some(InstructionInfo::new("PUSH23", 0, 1));
        arr[Opcode::PUSH24.as_usize()] = Some(InstructionInfo::new("PUSH24", 0, 1));
        arr[Opcode::PUSH25.as_usize()] = Some(InstructionInfo::new("PUSH25", 0, 1));
        arr[Opcode::PUSH26.as_usize()] = Some(InstructionInfo::new("PUSH26", 0, 1));
        arr[Opcode::PUSH27.as_usize()] = Some(InstructionInfo::new("PUSH27", 0, 1));
        arr[Opcode::PUSH28.as_usize()] = Some(InstructionInfo::new("PUSH28", 0, 1));
        arr[Opcode::PUSH29.as_usize()] = Some(InstructionInfo::new("PUSH29", 0, 1));
        arr[Opcode::PUSH30.as_usize()] = Some(InstructionInfo::new("PUSH30", 0, 1));
        arr[Opcode::PUSH31.as_usize()] = Some(InstructionInfo::new("PUSH31", 0, 1));
        arr[Opcode::PUSH32.as_usize()] = Some(InstructionInfo::new("PUSH32", 0, 1));
        arr[Opcode::DUP1.as_usize()] = Some(InstructionInfo::new("DUP1", 1, 2));
        arr[Opcode::DUP2.as_usize()] = Some(InstructionInfo::new("DUP2", 2, 3));
        arr[Opcode::DUP3.as_usize()] = Some(InstructionInfo::new("DUP3", 3, 4));
        arr[Opcode::DUP4.as_usize()] = Some(InstructionInfo::new("DUP4", 4, 5));
        arr[Opcode::DUP5.as_usize()] = Some(InstructionInfo::new("DUP5", 5, 6));
        arr[Opcode::DUP6.as_usize()] = Some(InstructionInfo::new("DUP6", 6, 7));
        arr[Opcode::DUP7.as_usize()] = Some(InstructionInfo::new("DUP7", 7, 8));
        arr[Opcode::DUP8.as_usize()] = Some(InstructionInfo::new("DUP8", 8, 9));
        arr[Opcode::DUP9.as_usize()] = Some(InstructionInfo::new("DUP9", 9, 10));
        arr[Opcode::DUP10.as_usize()] = Some(InstructionInfo::new("DUP10", 10, 11));
        arr[Opcode::DUP11.as_usize()] = Some(InstructionInfo::new("DUP11", 11, 12));
        arr[Opcode::DUP12.as_usize()] = Some(InstructionInfo::new("DUP12", 12, 13));
        arr[Opcode::DUP13.as_usize()] = Some(InstructionInfo::new("DUP13", 13, 14));
        arr[Opcode::DUP14.as_usize()] = Some(InstructionInfo::new("DUP14", 14, 15));
        arr[Opcode::DUP15.as_usize()] = Some(InstructionInfo::new("DUP15", 15, 16));
        arr[Opcode::DUP16.as_usize()] = Some(InstructionInfo::new("DUP16", 16, 17));
        arr[Opcode::SWAP1.as_usize()] = Some(InstructionInfo::new("SWAP1", 2, 2));
        arr[Opcode::SWAP2.as_usize()] = Some(InstructionInfo::new("SWAP2", 3, 3));
        arr[Opcode::SWAP3.as_usize()] = Some(InstructionInfo::new("SWAP3", 4, 4));
        arr[Opcode::SWAP4.as_usize()] = Some(InstructionInfo::new("SWAP4", 5, 5));
        arr[Opcode::SWAP5.as_usize()] = Some(InstructionInfo::new("SWAP5", 6, 6));
        arr[Opcode::SWAP6.as_usize()] = Some(InstructionInfo::new("SWAP6", 7, 7));
        arr[Opcode::SWAP7.as_usize()] = Some(InstructionInfo::new("SWAP7", 8, 8));
        arr[Opcode::SWAP8.as_usize()] = Some(InstructionInfo::new("SWAP8", 9, 9));
        arr[Opcode::SWAP9.as_usize()] = Some(InstructionInfo::new("SWAP9", 10, 10));
        arr[Opcode::SWAP10.as_usize()] = Some(InstructionInfo::new("SWAP10", 11, 11));
        arr[Opcode::SWAP11.as_usize()] = Some(InstructionInfo::new("SWAP11", 12, 12));
        arr[Opcode::SWAP12.as_usize()] = Some(InstructionInfo::new("SWAP12", 13, 13));
        arr[Opcode::SWAP13.as_usize()] = Some(InstructionInfo::new("SWAP13", 14, 14));
        arr[Opcode::SWAP14.as_usize()] = Some(InstructionInfo::new("SWAP14", 15, 15));
        arr[Opcode::SWAP15.as_usize()] = Some(InstructionInfo::new("SWAP15", 16, 16));
        arr[Opcode::SWAP16.as_usize()] = Some(InstructionInfo::new("SWAP16", 17, 17));
        arr[Opcode::LOG0.as_usize()] = Some(InstructionInfo::new("LOG0", 2, 0));
        arr[Opcode::LOG1.as_usize()] = Some(InstructionInfo::new("LOG1", 3, 0));
        arr[Opcode::LOG2.as_usize()] = Some(InstructionInfo::new("LOG2", 4, 0));
        arr[Opcode::LOG3.as_usize()] = Some(InstructionInfo::new("LOG3", 5, 0));
        arr[Opcode::LOG4.as_usize()] = Some(InstructionInfo::new("LOG4", 6, 0));
        //arr[Opcode::BEGINSUB.as_usize()] = Some(InstructionInfo::new("BEGINSUB", 0, 0));
        //arr[Opcode::JUMPSUB.as_usize()] = Some(InstructionInfo::new("JUMPSUB", 1, 0));
        //arr[Opcode::RETURNSUB.as_usize()] = Some(InstructionInfo::new("RETURNSUB", 0, 0));
        arr[Opcode::CREATE.as_usize()] = Some(InstructionInfo::new("CREATE", 3, 1));
        arr[Opcode::CALL.as_usize()] = Some(InstructionInfo::new("CALL", 7, 1));
        arr[Opcode::CALLCODE.as_usize()] = Some(InstructionInfo::new("CALLCODE", 7, 1));
        arr[Opcode::RETURN.as_usize()] = Some(InstructionInfo::new("RETURN", 2, 0));
        arr[Opcode::DELEGATECALL.as_usize()] = Some(InstructionInfo::new("DELEGATECALL", 6, 1));
        arr[Opcode::STATICCALL.as_usize()] = Some(InstructionInfo::new("STATICCALL", 6, 1));
        arr[Opcode::SUICIDE.as_usize()] = Some(InstructionInfo::new("SUICIDE", 1, 0));
        arr[Opcode::CREATE2.as_usize()] = Some(InstructionInfo::new("CREATE2", 4, 1));
        arr[Opcode::REVERT.as_usize()] = Some(InstructionInfo::new("REVERT", 2, 0));
        arr
    };
}

#[derive(serde::Serialize, Debug)]
pub struct TracedCall {
    pub vm_trace: Option<VMTrace>,
    pub full_trace_data: Vec<FullTraceData>,
    pub used_gas: u64,
    pub exit_reason: ExitReason,
}
