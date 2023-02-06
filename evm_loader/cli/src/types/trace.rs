use {
    crate::types::Bytes,
    ethnum::U256,
    std::collections::HashMap,
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some chunk of memory.
pub struct MemoryDiff {
    /// Offset into memory the change begins.
    pub offset: usize,
    /// The changed data.
    pub data: Bytes,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some storage value.
pub struct StorageDiff {
    /// Which key in storage is changed.
    pub location: U256,
    /// What the value has been changed to.
    pub value: [u8; 32],
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A record of an executed VM operation.
pub struct VMExecutedOperation {
    /// The total gas used.
    pub gas_used: U256,
    /// The stack item placed, if any.
    pub stack_push: Vec<[u8; 32]>,
    /// If altered, the memory delta.
    pub mem_diff: Option<MemoryDiff>,
    /// The altered storage value, if any.
    pub store_diff: Option<StorageDiff>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
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
#[allow(clippy::module_name_repetitions)]
pub struct TraceData {
    pub mem_written: Option<(usize, usize)>,
    pub store_written: Option<(U256, [u8; 32])>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct FullTraceData {
    pub stack: Vec<[u8; 32]>,
    pub memory: Vec<u8>,
    pub storage: HashMap<U256, [u8; 32]>,
}

/// Simple VM tracer. Traces all operations.
pub struct ExecutiveVMTracer {
    data: VMTrace,
    pub depth: usize,
    trace_stack: Vec<TraceData>,
}

impl ExecutiveVMTracer {
    /// Create a new top-level instance.
    pub fn toplevel() -> Self {
        ExecutiveVMTracer {
            data: VMTrace {
                parent_step: 0,
                code: vec![],
                operations: vec![VMOperation::default()], // prefill with a single entry so that prepare_subtrace can get the parent_step
                subs: vec![],
            },
            depth: 0,
            trace_stack: vec![],
        }
    }

    fn with_trace_in_depth<F: FnOnce(&mut VMTrace)>(trace: &mut VMTrace, depth: usize, f: F) {
        if depth == 0 {
            f(trace);
        } else {
            Self::with_trace_in_depth(trace.subs.last_mut().expect("self.depth is incremented with prepare_subtrace; a subtrace is always pushed; self.depth cannot be greater than subtrace stack; qed"), depth - 1, f);
        }
    }
}

impl VMTracer for ExecutiveVMTracer {
    type Output = VMTrace;

    fn trace_prepare_execute(&mut self, pc: usize, instruction: u8) {
        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            trace.operations.push(VMOperation {
                pc,
                instruction,
                gas_cost: U256::ZERO,
                executed: None,
            });
        });
    }

    fn trace_executed(&mut self, gas_used: U256, stack_push: Vec<[u8; 32]>, mem_diff: Option<MemoryDiff>, store_diff: Option<StorageDiff>) {
        self.trace_stack.push(TraceData {
            mem_written: mem_diff.as_ref().map(|d| (d.offset, d.data.len())),
            store_written: store_diff.as_ref().map(|d| (d.location, d.value))
        });

        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            let operation = trace.operations.last_mut().expect("trace_executed is always called after a trace_prepare_execute; trace.operations cannot be empty; qed");
            operation.executed = Some(VMExecutedOperation {
                gas_used,
                stack_push,
                mem_diff,
                store_diff,
            });
        });
    }

    fn prepare_subtrace(&mut self, code: Vec<u8>) {
        Self::with_trace_in_depth(&mut self.data, self.depth, move |trace| {
            let parent_step = trace.operations.len() - 1; // won't overflow since we must already have pushed an operation in trace_prepare_execute.
            trace.subs.push(VMTrace {
                parent_step,
                code,
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
    ) {
    }

    /// Trace the finalised execution of a single valid instruction.
    fn trace_executed(&mut self, _gas_used: U256, _stack_push: Vec<[u8; 32]>, _mem_diff: Option<MemoryDiff>, _storage_diff: Option<StorageDiff>) {}

    /// Spawn subtracer which will be used to trace deeper levels of execution.
    fn prepare_subtrace(&mut self, _code: Vec<u8>) {}

    /// Finalize subtracer.
    fn done_subtrace(&mut self) {}

    /// Consumes self and returns the VM trace.
    fn drain(self) -> Option<Self::Output>;
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TracedCall {
    pub vm_trace: Option<VMTrace>,
    pub full_trace_data: Vec<FullTraceData>,
    pub used_gas: u64,
}