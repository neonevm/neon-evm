use crate::types::Bytes;
use evm_loader::{H160, H256, U256};
use lazy_static::lazy_static;
use log::{debug, warn};
use std::cmp::min;
use std::fmt;
use std::sync::Arc;

type VmError = evm_loader::ExitReason;

pub type BlockNumber = u64;

/// Get the KECCAK (i.e. Keccak) hash of the empty bytes string.
pub const KECCAK_EMPTY: H256 = H256([
    0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
    0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
]);

/// Trace localized in vector of traces produced by a single transaction.
///
/// Parent and children indexes refer to positions in this vector.
#[derive(Debug, PartialEq, Clone)]
pub struct FlatTrace {
    /// Type of action performed by a transaction.
    pub action: Action,
    /// Result of this action.
    pub result: Res,
    /// Number of subtraces.
    pub subtraces: usize,
    /// Exact location of trace.
    ///
    /// [index in root, index in first CALL, index in second CALL, ...]
    pub trace_address: Vec<usize>,
}

/// Localized trace.
#[derive(Debug, PartialEq, Clone)]
pub struct LocalizedTrace {
    /// Type of action performed by a transaction.
    pub action: Action,
    /// Result of this action.
    pub result: Res,
    /// Number of subtraces.
    pub subtraces: usize,
    /// Exact location of trace.
    ///
    /// [index in root, index in first CALL, index in second CALL, ...]
    pub trace_address: Vec<usize>,
    /// Transaction number within the block.
    pub transaction_number: Option<usize>,
    /// Signed transaction hash.
    pub transaction_hash: Option<H256>,
    /// Block number.
    pub block_number: BlockNumber,
    /// Block hash.
    pub block_hash: H256,
}

/// Description of a _create_ action, either a `CREATE` operation or a create transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct Create {
    /// The address of the creator.
    pub from: H160,
    /// The value with which the new account is endowed.
    pub value: U256,
    /// The gas available for the creation init code.
    pub gas: U256,
    /// The init code.
    pub init: Bytes,
}

impl From<ActionParams> for Create {
    fn from(p: ActionParams) -> Self {
        Create {
            from: p.sender,
            value: p.value.value(),
            gas: p.gas,
            init: p.code.map_or_else(Vec::new, |c| (*c).clone()),
        }
    }
}

/// Suicide action.
#[derive(Debug, Clone, PartialEq)]
pub struct Suicide {
    /// Suicided address.
    pub address: H160,
    /// Suicided contract heir.
    pub refund_address: H160,
    /// Balance of the contract just before suicide.
    pub balance: U256,
}

/// Reward action
#[derive(Debug, Clone, PartialEq)]
pub struct Reward {
    /// Author's address.
    pub author: H160,
    /// Reward amount.
    pub value: U256,
    /// Reward type.
    pub reward_type: RewardType,
}

/// Reward type.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RewardType {
    /// Block
    Block,
    /// Uncle
    Uncle,
    /// Empty step (AuthorityRound)
    EmptyStep,
    /// A reward directly attributed by an external protocol (e.g. block reward contract)
    External,
}

/// Description of an action that we trace; will be either a call or a create.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// It's a call action.
    Call(Call),
    /// It's a create action.
    Create(Create),
    /// Suicide.
    Suicide(Suicide),
    /// Reward
    Reward(Reward),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActionType {
    /// It's a call action.
    Call,
    /// It's a create action.
    Create,
    /// Suicide.
    Suicide,
    /// Reward
    Reward,
}

/// The result of the performed action.
#[derive(Debug, Clone, PartialEq)]
pub enum Res {
    /// Successful call action result.
    Call(CallResult),
    /// Successful create action result.
    Create(CreateResult),
    /// Failed call.
    FailedCall(Error),
    /// Failed create.
    FailedCreate(Error),
    /// None
    None,
}

/// `Call` result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallResult {
    /// Gas used by call.
    pub gas_used: U256,
    /// Call Output.
    pub output: Bytes,
}

/// `Create` result.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateResult {
    /// Gas used by create.
    pub gas_used: U256,
    /// Code of the newly created contract.
    pub code: Bytes,
    /// H160 of the newly created contract.
    pub address: H160,
}

/// Description of a _call_ action, either a `CALL` operation or a message transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// The sending account.
    pub from: H160,
    /// The destination account.
    pub to: H160,
    /// The value transferred to the destination account.
    pub value: U256,
    /// The gas available for executing the call.
    pub gas: U256,
    /// The input data provided to the call.
    pub input: Bytes,
    /// The type of the call.
    pub call_type: CallType,
}

impl From<ActionParams> for Call {
    fn from(p: ActionParams) -> Self {
        match p.call_type {
            CallType::DelegateCall | CallType::CallCode => Call {
                from: p.address,
                to: p.code_address,
                value: p.value.value(),
                gas: p.gas,
                input: p.data.unwrap_or_else(Vec::new),
                call_type: p.call_type,
            },
            _ => Call {
                from: p.sender,
                to: p.address,
                value: p.value.value(),
                gas: p.gas,
                input: p.data.unwrap_or_else(Vec::new),
                call_type: p.call_type,
            },
        }
    }
}

/// Trace evm errors.
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// `OutOfGas` is returned when transaction execution runs out of gas.
    OutOfGas,
    /// `BadJumpDestination` is returned when execution tried to move
    /// to position that wasn't marked with JUMPDEST instruction
    BadJumpDestination,
    /// `BadInstructions` is returned when given instruction is not supported
    BadInstruction,
    /// `StackUnderflow` when there is not enough stack elements to execute instruction
    StackUnderflow,
    /// When execution would exceed defined Stack Limit
    OutOfStack,
    /// When there is not enough subroutine stack elements to return from
    SubStackUnderflow,
    /// When execution would exceed defined subroutine Stack Limit
    OutOfSubStack,
    /// When the code walks into a subroutine, that is not allowed
    InvalidSubEntry,
    /// When builtin contract failed on input data
    BuiltIn,
    /// Returned on evm internal error. Should never be ignored during development.
    /// Likely to cause consensus issues.
    Internal,
    /// When execution tries to modify the state in static context
    MutableCallInStaticContext,
    /// When invalid code was attempted to deploy
    InvalidCode,
    /// Wasm error
    Wasm,
    /// Contract tried to access past the return data buffer.
    OutOfBounds,
    /// Execution has been reverted with REVERT instruction.
    Reverted,
}

impl<'a> From<&'a evm_loader::ExitReason> for Error {
    fn from(reason: &'a evm_loader::ExitReason) -> Self {
        use evm_loader::ExitError;
        use evm_loader::ExitReason;
        match reason {
            ExitReason::Error(error) => match error {
                ExitError::OutOfGas => Error::OutOfGas,
                ExitError::StackOverflow => Error::OutOfStack,
                ExitError::StackUnderflow => Error::StackUnderflow,
                ExitError::InvalidJump => Error::BadJumpDestination,
                ExitError::InvalidRange => Error::OutOfBounds, // TODO ???
                ExitError::DesignatedInvalid => Error::BadInstruction, // TODO ???
                ExitError::CallTooDeep => Error::OutOfStack,
                ExitError::OutOfOffset => Error::OutOfBounds,
                ExitError::OutOfFund => todo!(),
                ExitError::PCUnderflow => todo!(),
                ExitError::CreateEmpty => todo!(),
                ExitError::CreateCollision => todo!(),
                ExitError::CreateContractLimit => todo!(),
                ExitError::StaticModeViolation => todo!(),
            },
            ExitReason::Revert(_) => Error::Reverted,
            ExitReason::Succeed(_) => todo!("not expected to succeed"),
            ExitReason::Fatal(_) => todo!(),
            ExitReason::StepLimitReached => todo!(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        let message = match *self {
            OutOfGas => "Out of gas",
            BadJumpDestination => "Bad jump destination",
            BadInstruction => "Bad instruction",
            StackUnderflow => "Stack underflow",
            OutOfStack => "Out of stack",
            SubStackUnderflow => "Subroutine stack underflow",
            OutOfSubStack => "Subroutine stack overflow",
            BuiltIn => "Built-in failed",
            InvalidSubEntry => "Invalid subroutine entry",
            InvalidCode => "Invalid code",
            Wasm => "Wasm runtime error",
            Internal => "Internal error",
            MutableCallInStaticContext => "Mutable Call In Static Context",
            OutOfBounds => "Out of bounds",
            Reverted => "Reverted",
        };
        message.fmt(f)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CallType {
    /// Not a CALL.
    None,
    /// CALL.
    Call,
    /// CALLCODE.
    CallCode,
    /// DELEGATECALL.
    DelegateCall,
    /// STATICCALL
    StaticCall,
}

#[derive(Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some chunk of memory.
pub struct MemoryDiff {
    /// Offset into memory the change begins.
    pub offset: usize,
    /// The changed data.
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
/// A diff of some storage value.
pub struct StorageDiff {
    /// Which key in storage is changed.
    pub location: U256,
    /// What the value has been changed to.
    pub value: U256,
}

#[derive(Debug, Clone, PartialEq /*, RlpEncodable, RlpDecodable */)]
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

#[derive(Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
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

#[derive(Debug, Clone, PartialEq, Default /*, RlpEncodable, RlpDecodable */)]
/// A record of a full VM trace for a CALL/CREATE.
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

#[derive(Clone, Debug)]
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
    pub fn toplevel() -> Self {
        ExecutiveVMTracer {
            data: VMTrace {
                parent_step: 0,
                code: vec![].into(),
                operations: vec![Default::default()], // prefill with a single entry so that prepare_subtrace can get the parent_step
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

    fn trace_next_instruction(&mut self, _pc: usize, _instruction: u8, _current_gas: U256) -> bool {
        true
    }

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
    /// Data returned when draining the VMTracer.
    type Output;

    /// Trace the progression of interpreter to next instruction.
    /// If tracer returns `false` it won't be called again.
    /// @returns true if `trace_prepare_execute` and `trace_executed` should be called.
    fn trace_next_instruction(&mut self, _pc: usize, _instruction: u8, _current_gas: U256) -> bool {
        false
    }

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

/// Transaction value
#[derive(Clone, Debug)]
pub enum ActionValue {
    /// Value that should be transfered
    Transfer(U256),
    /// Apparent value for transaction (not transfered)
    Apparent(U256),
}

/// Type of the way parameters encoded
#[derive(Clone, Debug)]
pub enum ParamsType {
    /// Parameters are included in code
    Embedded,
    /// Parameters are passed in data section
    Separate,
}

impl ActionValue {
    /// Returns action value as U256.
    pub fn value(&self) -> U256 {
        match *self {
            ActionValue::Transfer(x) | ActionValue::Apparent(x) => x,
        }
    }

    /// Returns the transfer action value of the U256-convertable raw value
    pub fn transfer<T: Into<U256>>(transfer_value: T) -> ActionValue {
        ActionValue::Transfer(transfer_value.into())
    }

    /// Returns the apparent action value of the U256-convertable raw value
    pub fn apparent<T: Into<U256>>(apparent_value: T) -> ActionValue {
        ActionValue::Apparent(apparent_value.into())
    }
}

// TODO: should be a trait, possible to avoid cloning everything from a Transaction(/View).
/// Action (call/create) input params. Everything else should be specified in Externalities.
#[derive(Clone, Debug)]
pub struct ActionParams {
    /// H160 of currently executed code.
    pub code_address: H160,
    /// Hash of currently executed code.
    pub code_hash: Option<H256>,
    /// Receive address. Usually equal to code_address,
    /// except when called using CALLCODE.
    pub address: H160,
    /// Sender of current part of the transaction.
    pub sender: H160,
    /// Transaction initiator.
    pub origin: H160,
    /// Gas paid up front for transaction execution
    pub gas: U256,
    /// Gas price.
    pub gas_price: U256,
    /// Transaction value.
    pub value: ActionValue,
    /// Code being executed.
    pub code: Option<Arc<Bytes>>,
    /// Input data.
    pub data: Option<Bytes>,
    /// Type of call
    pub call_type: CallType,
    /// Param types encoding
    pub params_type: ParamsType,
    // /// Current access list
    pub access_list: AccessList,
}

impl Default for ActionParams {
    /// Returns default ActionParams initialized with zeros
    fn default() -> ActionParams {
        ActionParams {
            code_address: H160::default(),
            code_hash: Some(KECCAK_EMPTY),
            address: H160::default(),
            sender: H160::default(),
            origin: H160::default(),
            gas: U256::zero(),
            gas_price: U256::zero(),
            value: ActionValue::Transfer(U256::zero()),
            code: None,
            data: None,
            call_type: CallType::None,
            params_type: ParamsType::Separate,
            access_list: AccessList::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessList {
    id: usize,
    //journal: Rc<RefCell<Journal>>,
}

impl Default for AccessList {
    fn default() -> Self {
        Self { id: 0 }
    }
}

/// This trait is used by executive to build traces.
pub trait Tracer: Send {
    /// Data returned when draining the Tracer.
    type Output;

    fn last_action_type(&self) -> ActionType;

    /// Prepares call trace for given params. Would panic if prepare/done_trace are not balanced.
    fn prepare_trace_call(&mut self, params: Call, depth: usize, is_builtin: bool);

    /// Prepares create trace for given params. Would panic if prepare/done_trace are not balanced.
    fn prepare_trace_create(&mut self, params: Create, address: H160);

    /// Finishes a successful call trace. Would panic if prepare/done_trace are not balanced.
    fn done_trace_call(&mut self, gas_used: U256, output: &[u8]);

    /// Finishes a successful create trace. Would panic if prepare/done_trace are not balanced.
    fn done_trace_create(&mut self, gas_used: U256, code: &[u8]);

    /// Finishes a failed trace. Would panic if prepare/done_trace are not balanced.
    fn done_trace_failed(&mut self, error: &VmError);

    /// Stores suicide info.
    fn trace_suicide(&mut self, address: H160, balance: U256, refund_address: H160);

    /// Stores reward info.
    fn trace_reward(&mut self, author: H160, value: U256, reward_type: RewardType);

    /// Consumes self and returns all traces.
    fn drain(self) -> Vec<Self::Output>;
}

/// Simple executive tracer. Traces all calls and creates. Ignores delegatecalls.
#[derive(Default)]
pub struct ExecutiveTracer {
    traces: Vec<FlatTrace>,
    index_stack: Vec<usize>,
    vecindex_stack: Vec<usize>,
    sublen_stack: Vec<usize>,
    skip_one: bool,
    // gas_snapshot: Option<Snapshot>,
}

impl ExecutiveTracer {
    // pub fn set_snapshot(&mut self, new: Snapshot) {
    //     self.gas_snapshot.replace(new);
    // }

    fn get_current_used_gas(&self) -> U256 {
        // self.gas_snapshot
        //     .map_or(0, |snapshot| snapshot.used_gas)
        //     .into()
        U256::zero()
    }
}

impl Tracer for ExecutiveTracer {
    type Output = FlatTrace;

    fn prepare_trace_call(&mut self, params: Call, depth: usize, is_builtin: bool) {
        assert!(!self.skip_one, "skip_one is used only for builtin contracts that do not have subsequent calls; in prepare_trace_call it cannot be true; qed");

        if depth != 0 && is_builtin && params.value == U256::zero() {
            self.skip_one = true;
            return;
        }

        if let Some(parentlen) = self.sublen_stack.last_mut() {
            *parentlen += 1;
        }

        let trace = FlatTrace {
            trace_address: self.index_stack.clone(),
            subtraces: self.sublen_stack.last().cloned().unwrap_or(0),
            action: Action::Call(Call::from(params.clone())),
            result: Res::Call(CallResult {
                // Will be updated at exit
                gas_used: U256::zero(),
                output: Vec::new(),
            }),
        };
        self.vecindex_stack.push(self.traces.len());
        self.traces.push(trace);
        self.index_stack.push(0);
        self.sublen_stack.push(0);
    }

    fn prepare_trace_create(&mut self, params: Create, address: H160) {
        assert!(!self.skip_one, "skip_one is used only for builtin contracts that do not have subsequent calls; in prepare_trace_create it cannot be true; qed");

        if let Some(parentlen) = self.sublen_stack.last_mut() {
            *parentlen += 1;
        }

        let trace = FlatTrace {
            trace_address: self.index_stack.clone(),
            subtraces: self.sublen_stack.last().cloned().unwrap_or(0),
            action: Action::Create(Create::from(params.clone())),
            result: Res::Create(CreateResult {
                gas_used: U256::zero(),
                code: Vec::new(),
                address,
            }),
        };
        self.vecindex_stack.push(self.traces.len());
        self.traces.push(trace);
        self.index_stack.push(0);
        self.sublen_stack.push(0);
    }

    fn last_action_type(&self) -> ActionType {
        let vecindex = self.vecindex_stack.last().expect("prepared");

        match self.traces[*vecindex].action {
            Action::Call(..) => ActionType::Call,
            Action::Create(..) => ActionType::Create,
            Action::Reward(..) => ActionType::Reward,
            Action::Suicide(..) => ActionType::Suicide,
        }
    }

    fn done_trace_call(&mut self, _gas_used: U256, res_output: &[u8]) {
        if self.skip_one {
            self.skip_one = false;
            return;
        }

        let vecindex = self.vecindex_stack.pop().expect("Executive invoked prepare_trace_call before this function; vecindex_stack is never empty; qed");
        let sublen = self.sublen_stack.pop().expect("Executive invoked prepare_trace_call before this function; sublen_stack is never empty; qed");
        self.index_stack.pop();

        let current_used = self.get_current_used_gas();
        match &mut self.traces[vecindex].result {
            Res::Call(CallResult {
                ref mut gas_used,
                ref mut output,
                ..
            }) => {
                *output = res_output.into();
                *gas_used = current_used - *gas_used;
            }
            _ => panic!("this cant happen"),
        };

        self.traces[vecindex].subtraces = sublen;

        if let Some(index) = self.index_stack.last_mut() {
            *index += 1;
        }
    }

    fn done_trace_create(&mut self, _gas_used: U256, res_code: &[u8]) {
        assert!(!self.skip_one, "skip_one is only set with prepare_trace_call for builtin contracts with no subsequent calls; skip_one cannot be true after the same level prepare_trace_create; qed");

        let vecindex = self.vecindex_stack.pop().expect("Executive invoked prepare_trace_create before this function; vecindex_stack is never empty; qed");
        let sublen = self.sublen_stack.pop().expect("Executive invoked prepare_trace_create before this function; sublen_stack is never empty; qed");
        self.index_stack.pop();

        let current_used = self.get_current_used_gas();
        match &mut self.traces[vecindex].result {
            Res::Create(CreateResult {
                ref mut gas_used,
                ref mut code,
                ..
            }) => {
                *code = res_code.into();
                *gas_used = current_used - *gas_used;
            }
            _ => panic!("this cant happen"),
        };
        self.traces[vecindex].subtraces = sublen;

        if let Some(index) = self.index_stack.last_mut() {
            *index += 1;
        }
    }

    fn done_trace_failed(&mut self, error: &VmError) {
        if self.skip_one {
            self.skip_one = false;
            return;
        }

        let vecindex = self.vecindex_stack.pop().expect("Executive invoked prepare_trace_create/call before this function; vecindex_stack is never empty; qed");
        let sublen = self.sublen_stack.pop().expect("Executive invoked prepare_trace_create/call before this function; vecindex_stack is never empty; qed");
        self.index_stack.pop();

        let is_create = match self.traces[vecindex].action {
            Action::Create(_) => true,
            _ => false,
        };

        if is_create {
            self.traces[vecindex].result = Res::FailedCreate(error.into());
        } else {
            self.traces[vecindex].result = Res::FailedCall(error.into());
        }
        self.traces[vecindex].subtraces = sublen;

        if let Some(index) = self.index_stack.last_mut() {
            *index += 1;
        }
    }

    fn trace_suicide(&mut self, address: H160, balance: U256, refund_address: H160) {
        if let Some(parentlen) = self.sublen_stack.last_mut() {
            *parentlen += 1;
        }

        let trace = FlatTrace {
            subtraces: 0,
            action: Action::Suicide(Suicide {
                address,
                refund_address,
                balance,
            }),
            result: Res::None,
            trace_address: self.index_stack.clone(),
        };
        debug!(target: "trace", "Traced suicide {:?}", trace);
        self.traces.push(trace);

        if let Some(index) = self.index_stack.last_mut() {
            *index += 1;
        }
    }

    fn trace_reward(&mut self, author: H160, value: U256, reward_type: RewardType) {
        if let Some(parentlen) = self.sublen_stack.last_mut() {
            *parentlen += 1;
        }

        let trace = FlatTrace {
            subtraces: 0,
            action: Action::Reward(Reward {
                author,
                value,
                reward_type,
            }),
            result: Res::None,
            trace_address: self.index_stack.clone(),
        };
        debug!(target: "trace", "Traced reward {:?}", trace);
        self.traces.push(trace);

        if let Some(index) = self.index_stack.last_mut() {
            *index += 1;
        }
    }

    fn drain(self) -> Vec<FlatTrace> {
        self.traces
    }
}

// openethereum/crates/vm/evm/src/instructions.rs
#[derive(Copy, Clone)]
pub struct InstructionInfo {
    /// Mnemonic name.
    pub name: &'static str,
    /// Number of stack arguments.
    pub args: usize,
    /// Number of returned stack items.
    pub ret: usize,
    /// Gas price tier.
    pub tier: GasPriceTier,
}

impl InstructionInfo {
    /// Create new instruction info.
    pub fn new(name: &'static str, args: usize, ret: usize, tier: GasPriceTier) -> Self {
        InstructionInfo {
            name,
            args,
            ret,
            tier,
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum GasPriceTier {
    /// 0 Zero
    Zero,
    /// 2 Quick
    Base,
    /// 3 Fastest
    VeryLow,
    /// 5 Fast
    Low,
    /// 8 Mid
    Mid,
    /// 10 Slow
    High,
    /// 20 Ext
    Ext,
    /// Multiparam or otherwise special
    Special,
}

lazy_static! {
    /// Static instruction table.
    pub static ref INSTRUCTIONS: [Option<InstructionInfo>; 0x100] = {
        use evm_loader::Opcode;
        let mut arr = [None; 0x100];
        arr[Opcode::STOP.as_usize()] = Some(InstructionInfo::new("STOP", 0, 0, GasPriceTier::Zero));
        arr[Opcode::ADD.as_usize()] = Some(InstructionInfo::new("ADD", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SUB.as_usize()] = Some(InstructionInfo::new("SUB", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::MUL.as_usize()] = Some(InstructionInfo::new("MUL", 2, 1, GasPriceTier::Low));
        arr[Opcode::DIV.as_usize()] = Some(InstructionInfo::new("DIV", 2, 1, GasPriceTier::Low));
        arr[Opcode::SDIV.as_usize()] = Some(InstructionInfo::new("SDIV", 2, 1, GasPriceTier::Low));
        arr[Opcode::MOD.as_usize()] = Some(InstructionInfo::new("MOD", 2, 1, GasPriceTier::Low));
        arr[Opcode::SMOD.as_usize()] = Some(InstructionInfo::new("SMOD", 2, 1, GasPriceTier::Low));
        arr[Opcode::EXP.as_usize()] = Some(InstructionInfo::new("EXP", 2, 1, GasPriceTier::Special));
        arr[Opcode::NOT.as_usize()] = Some(InstructionInfo::new("NOT", 1, 1, GasPriceTier::VeryLow));
        arr[Opcode::LT.as_usize()] = Some(InstructionInfo::new("LT", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::GT.as_usize()] = Some(InstructionInfo::new("GT", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SLT.as_usize()] = Some(InstructionInfo::new("SLT", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SGT.as_usize()] = Some(InstructionInfo::new("SGT", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::EQ.as_usize()] = Some(InstructionInfo::new("EQ", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::ISZERO.as_usize()] = Some(InstructionInfo::new("ISZERO", 1, 1, GasPriceTier::VeryLow));
        arr[Opcode::AND.as_usize()] = Some(InstructionInfo::new("AND", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::OR.as_usize()] = Some(InstructionInfo::new("OR", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::XOR.as_usize()] = Some(InstructionInfo::new("XOR", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::BYTE.as_usize()] = Some(InstructionInfo::new("BYTE", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SHL.as_usize()] = Some(InstructionInfo::new("SHL", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SHR.as_usize()] = Some(InstructionInfo::new("SHR", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::SAR.as_usize()] = Some(InstructionInfo::new("SAR", 2, 1, GasPriceTier::VeryLow));
        arr[Opcode::ADDMOD.as_usize()] = Some(InstructionInfo::new("ADDMOD", 3, 1, GasPriceTier::Mid));
        arr[Opcode::MULMOD.as_usize()] = Some(InstructionInfo::new("MULMOD", 3, 1, GasPriceTier::Mid));
        arr[Opcode::SIGNEXTEND.as_usize()] = Some(InstructionInfo::new("SIGNEXTEND", 2, 1, GasPriceTier::Low));
        arr[Opcode::RETURNDATASIZE.as_usize()] = Some(InstructionInfo::new("RETURNDATASIZE", 0, 1, GasPriceTier::Base));
        arr[Opcode::RETURNDATACOPY.as_usize()] = Some(InstructionInfo::new("RETURNDATACOPY", 3, 0, GasPriceTier::VeryLow));
        arr[Opcode::SHA3.as_usize()] = Some(InstructionInfo::new("SHA3", 2, 1, GasPriceTier::Special));
        arr[Opcode::ADDRESS.as_usize()] = Some(InstructionInfo::new("ADDRESS", 0, 1, GasPriceTier::Base));
        arr[Opcode::BALANCE.as_usize()] = Some(InstructionInfo::new("BALANCE", 1, 1, GasPriceTier::Special));
        arr[Opcode::ORIGIN.as_usize()] = Some(InstructionInfo::new("ORIGIN", 0, 1, GasPriceTier::Base));
        arr[Opcode::CALLER.as_usize()] = Some(InstructionInfo::new("CALLER", 0, 1, GasPriceTier::Base));
        arr[Opcode::CALLVALUE.as_usize()] = Some(InstructionInfo::new("CALLVALUE", 0, 1, GasPriceTier::Base));
        arr[Opcode::CALLDATALOAD.as_usize()] = Some(InstructionInfo::new("CALLDATALOAD", 1, 1, GasPriceTier::VeryLow));
        arr[Opcode::CALLDATASIZE.as_usize()] = Some(InstructionInfo::new("CALLDATASIZE", 0, 1, GasPriceTier::Base));
        arr[Opcode::CALLDATACOPY.as_usize()] = Some(InstructionInfo::new("CALLDATACOPY", 3, 0, GasPriceTier::VeryLow));
        arr[Opcode::EXTCODEHASH.as_usize()] = Some(InstructionInfo::new("EXTCODEHASH", 1, 1, GasPriceTier::Special));
        arr[Opcode::CODESIZE.as_usize()] = Some(InstructionInfo::new("CODESIZE", 0, 1, GasPriceTier::Base));
        arr[Opcode::CODECOPY.as_usize()] = Some(InstructionInfo::new("CODECOPY", 3, 0, GasPriceTier::VeryLow));
        arr[Opcode::GASPRICE.as_usize()] = Some(InstructionInfo::new("GASPRICE", 0, 1, GasPriceTier::Base));
        arr[Opcode::EXTCODESIZE.as_usize()] = Some(InstructionInfo::new("EXTCODESIZE", 1, 1, GasPriceTier::Special));
        arr[Opcode::EXTCODECOPY.as_usize()] = Some(InstructionInfo::new("EXTCODECOPY", 4, 0, GasPriceTier::Special));
        arr[Opcode::BLOCKHASH.as_usize()] = Some(InstructionInfo::new("BLOCKHASH", 1, 1, GasPriceTier::Ext));
        arr[Opcode::COINBASE.as_usize()] = Some(InstructionInfo::new("COINBASE", 0, 1, GasPriceTier::Base));
        arr[Opcode::TIMESTAMP.as_usize()] = Some(InstructionInfo::new("TIMESTAMP", 0, 1, GasPriceTier::Base));
        arr[Opcode::NUMBER.as_usize()] = Some(InstructionInfo::new("NUMBER", 0, 1, GasPriceTier::Base));
        arr[Opcode::DIFFICULTY.as_usize()] = Some(InstructionInfo::new("DIFFICULTY", 0, 1, GasPriceTier::Base));
        arr[Opcode::GASLIMIT.as_usize()] = Some(InstructionInfo::new("GASLIMIT", 0, 1, GasPriceTier::Base));
        arr[Opcode::CHAINID.as_usize()] = Some(InstructionInfo::new("CHAINID", 0, 1, GasPriceTier::Base));
        arr[Opcode::SELFBALANCE.as_usize()] = Some(InstructionInfo::new("SELFBALANCE", 0, 1, GasPriceTier::Low));
        //arr[Opcode::BASEFEE.as_usize()] = Some(InstructionInfo::new("BASEFEE", 0, 1, GasPriceTier::Base));
        arr[Opcode::POP.as_usize()] = Some(InstructionInfo::new("POP", 1, 0, GasPriceTier::Base));
        arr[Opcode::MLOAD.as_usize()] = Some(InstructionInfo::new("MLOAD", 1, 1, GasPriceTier::VeryLow));
        arr[Opcode::MSTORE.as_usize()] = Some(InstructionInfo::new("MSTORE", 2, 0, GasPriceTier::VeryLow));
        arr[Opcode::MSTORE8.as_usize()] = Some(InstructionInfo::new("MSTORE8", 2, 0, GasPriceTier::VeryLow));
        arr[Opcode::SLOAD.as_usize()] = Some(InstructionInfo::new("SLOAD", 1, 1, GasPriceTier::Special));
        arr[Opcode::SSTORE.as_usize()] = Some(InstructionInfo::new("SSTORE", 2, 0, GasPriceTier::Special));
        arr[Opcode::JUMP.as_usize()] = Some(InstructionInfo::new("JUMP", 1, 0, GasPriceTier::Mid));
        arr[Opcode::JUMPI.as_usize()] = Some(InstructionInfo::new("JUMPI", 2, 0, GasPriceTier::High));
        arr[Opcode::PC.as_usize()] = Some(InstructionInfo::new("PC", 0, 1, GasPriceTier::Base));
        arr[Opcode::MSIZE.as_usize()] = Some(InstructionInfo::new("MSIZE", 0, 1, GasPriceTier::Base));
        arr[Opcode::GAS.as_usize()] = Some(InstructionInfo::new("GAS", 0, 1, GasPriceTier::Base));
        arr[Opcode::JUMPDEST.as_usize()] = Some(InstructionInfo::new("JUMPDEST", 0, 0, GasPriceTier::Special));
        arr[Opcode::PUSH1.as_usize()] = Some(InstructionInfo::new("PUSH1", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH2.as_usize()] = Some(InstructionInfo::new("PUSH2", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH3.as_usize()] = Some(InstructionInfo::new("PUSH3", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH4.as_usize()] = Some(InstructionInfo::new("PUSH4", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH5.as_usize()] = Some(InstructionInfo::new("PUSH5", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH6.as_usize()] = Some(InstructionInfo::new("PUSH6", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH7.as_usize()] = Some(InstructionInfo::new("PUSH7", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH8.as_usize()] = Some(InstructionInfo::new("PUSH8", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH9.as_usize()] = Some(InstructionInfo::new("PUSH9", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH10.as_usize()] = Some(InstructionInfo::new("PUSH10", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH11.as_usize()] = Some(InstructionInfo::new("PUSH11", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH12.as_usize()] = Some(InstructionInfo::new("PUSH12", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH13.as_usize()] = Some(InstructionInfo::new("PUSH13", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH14.as_usize()] = Some(InstructionInfo::new("PUSH14", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH15.as_usize()] = Some(InstructionInfo::new("PUSH15", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH16.as_usize()] = Some(InstructionInfo::new("PUSH16", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH17.as_usize()] = Some(InstructionInfo::new("PUSH17", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH18.as_usize()] = Some(InstructionInfo::new("PUSH18", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH19.as_usize()] = Some(InstructionInfo::new("PUSH19", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH20.as_usize()] = Some(InstructionInfo::new("PUSH20", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH21.as_usize()] = Some(InstructionInfo::new("PUSH21", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH22.as_usize()] = Some(InstructionInfo::new("PUSH22", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH23.as_usize()] = Some(InstructionInfo::new("PUSH23", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH24.as_usize()] = Some(InstructionInfo::new("PUSH24", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH25.as_usize()] = Some(InstructionInfo::new("PUSH25", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH26.as_usize()] = Some(InstructionInfo::new("PUSH26", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH27.as_usize()] = Some(InstructionInfo::new("PUSH27", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH28.as_usize()] = Some(InstructionInfo::new("PUSH28", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH29.as_usize()] = Some(InstructionInfo::new("PUSH29", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH30.as_usize()] = Some(InstructionInfo::new("PUSH30", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH31.as_usize()] = Some(InstructionInfo::new("PUSH31", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::PUSH32.as_usize()] = Some(InstructionInfo::new("PUSH32", 0, 1, GasPriceTier::VeryLow));
        arr[Opcode::DUP1.as_usize()] = Some(InstructionInfo::new("DUP1", 1, 2, GasPriceTier::VeryLow));
        arr[Opcode::DUP2.as_usize()] = Some(InstructionInfo::new("DUP2", 2, 3, GasPriceTier::VeryLow));
        arr[Opcode::DUP3.as_usize()] = Some(InstructionInfo::new("DUP3", 3, 4, GasPriceTier::VeryLow));
        arr[Opcode::DUP4.as_usize()] = Some(InstructionInfo::new("DUP4", 4, 5, GasPriceTier::VeryLow));
        arr[Opcode::DUP5.as_usize()] = Some(InstructionInfo::new("DUP5", 5, 6, GasPriceTier::VeryLow));
        arr[Opcode::DUP6.as_usize()] = Some(InstructionInfo::new("DUP6", 6, 7, GasPriceTier::VeryLow));
        arr[Opcode::DUP7.as_usize()] = Some(InstructionInfo::new("DUP7", 7, 8, GasPriceTier::VeryLow));
        arr[Opcode::DUP8.as_usize()] = Some(InstructionInfo::new("DUP8", 8, 9, GasPriceTier::VeryLow));
        arr[Opcode::DUP9.as_usize()] = Some(InstructionInfo::new("DUP9", 9, 10, GasPriceTier::VeryLow));
        arr[Opcode::DUP10.as_usize()] = Some(InstructionInfo::new("DUP10", 10, 11, GasPriceTier::VeryLow));
        arr[Opcode::DUP11.as_usize()] = Some(InstructionInfo::new("DUP11", 11, 12, GasPriceTier::VeryLow));
        arr[Opcode::DUP12.as_usize()] = Some(InstructionInfo::new("DUP12", 12, 13, GasPriceTier::VeryLow));
        arr[Opcode::DUP13.as_usize()] = Some(InstructionInfo::new("DUP13", 13, 14, GasPriceTier::VeryLow));
        arr[Opcode::DUP14.as_usize()] = Some(InstructionInfo::new("DUP14", 14, 15, GasPriceTier::VeryLow));
        arr[Opcode::DUP15.as_usize()] = Some(InstructionInfo::new("DUP15", 15, 16, GasPriceTier::VeryLow));
        arr[Opcode::DUP16.as_usize()] = Some(InstructionInfo::new("DUP16", 16, 17, GasPriceTier::VeryLow));
        arr[Opcode::SWAP1.as_usize()] = Some(InstructionInfo::new("SWAP1", 2, 2, GasPriceTier::VeryLow));
        arr[Opcode::SWAP2.as_usize()] = Some(InstructionInfo::new("SWAP2", 3, 3, GasPriceTier::VeryLow));
        arr[Opcode::SWAP3.as_usize()] = Some(InstructionInfo::new("SWAP3", 4, 4, GasPriceTier::VeryLow));
        arr[Opcode::SWAP4.as_usize()] = Some(InstructionInfo::new("SWAP4", 5, 5, GasPriceTier::VeryLow));
        arr[Opcode::SWAP5.as_usize()] = Some(InstructionInfo::new("SWAP5", 6, 6, GasPriceTier::VeryLow));
        arr[Opcode::SWAP6.as_usize()] = Some(InstructionInfo::new("SWAP6", 7, 7, GasPriceTier::VeryLow));
        arr[Opcode::SWAP7.as_usize()] = Some(InstructionInfo::new("SWAP7", 8, 8, GasPriceTier::VeryLow));
        arr[Opcode::SWAP8.as_usize()] = Some(InstructionInfo::new("SWAP8", 9, 9, GasPriceTier::VeryLow));
        arr[Opcode::SWAP9.as_usize()] = Some(InstructionInfo::new("SWAP9", 10, 10, GasPriceTier::VeryLow));
        arr[Opcode::SWAP10.as_usize()] = Some(InstructionInfo::new("SWAP10", 11, 11, GasPriceTier::VeryLow));
        arr[Opcode::SWAP11.as_usize()] = Some(InstructionInfo::new("SWAP11", 12, 12, GasPriceTier::VeryLow));
        arr[Opcode::SWAP12.as_usize()] = Some(InstructionInfo::new("SWAP12", 13, 13, GasPriceTier::VeryLow));
        arr[Opcode::SWAP13.as_usize()] = Some(InstructionInfo::new("SWAP13", 14, 14, GasPriceTier::VeryLow));
        arr[Opcode::SWAP14.as_usize()] = Some(InstructionInfo::new("SWAP14", 15, 15, GasPriceTier::VeryLow));
        arr[Opcode::SWAP15.as_usize()] = Some(InstructionInfo::new("SWAP15", 16, 16, GasPriceTier::VeryLow));
        arr[Opcode::SWAP16.as_usize()] = Some(InstructionInfo::new("SWAP16", 17, 17, GasPriceTier::VeryLow));
        arr[Opcode::LOG0.as_usize()] = Some(InstructionInfo::new("LOG0", 2, 0, GasPriceTier::Special));
        arr[Opcode::LOG1.as_usize()] = Some(InstructionInfo::new("LOG1", 3, 0, GasPriceTier::Special));
        arr[Opcode::LOG2.as_usize()] = Some(InstructionInfo::new("LOG2", 4, 0, GasPriceTier::Special));
        arr[Opcode::LOG3.as_usize()] = Some(InstructionInfo::new("LOG3", 5, 0, GasPriceTier::Special));
        arr[Opcode::LOG4.as_usize()] = Some(InstructionInfo::new("LOG4", 6, 0, GasPriceTier::Special));
        //arr[Opcode::BEGINSUB.as_usize()] = Some(InstructionInfo::new("BEGINSUB", 0, 0, GasPriceTier::Base));
        //arr[Opcode::JUMPSUB.as_usize()] = Some(InstructionInfo::new("JUMPSUB", 1, 0, GasPriceTier::High));
        //arr[Opcode::RETURNSUB.as_usize()] = Some(InstructionInfo::new("RETURNSUB", 0, 0, GasPriceTier::Low));
        arr[Opcode::CREATE.as_usize()] = Some(InstructionInfo::new("CREATE", 3, 1, GasPriceTier::Special));
        arr[Opcode::CALL.as_usize()] = Some(InstructionInfo::new("CALL", 7, 1, GasPriceTier::Special));
        arr[Opcode::CALLCODE.as_usize()] = Some(InstructionInfo::new("CALLCODE", 7, 1, GasPriceTier::Special));
        arr[Opcode::RETURN.as_usize()] = Some(InstructionInfo::new("RETURN", 2, 0, GasPriceTier::Zero));
        arr[Opcode::DELEGATECALL.as_usize()] = Some(InstructionInfo::new("DELEGATECALL", 6, 1, GasPriceTier::Special));
        arr[Opcode::STATICCALL.as_usize()] = Some(InstructionInfo::new("STATICCALL", 6, 1, GasPriceTier::Special));
        arr[Opcode::SUICIDE.as_usize()] = Some(InstructionInfo::new("SUICIDE", 1, 0, GasPriceTier::Special));
        arr[Opcode::CREATE2.as_usize()] = Some(InstructionInfo::new("CREATE2", 4, 1, GasPriceTier::Special));
        arr[Opcode::REVERT.as_usize()] = Some(InstructionInfo::new("REVERT", 2, 0, GasPriceTier::Zero));
        arr
    };
}
