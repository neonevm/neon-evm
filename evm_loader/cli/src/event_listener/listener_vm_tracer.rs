use evm_loader::{
    U256, Stack, ExitReason, Capture, StepTrace, StepResultTrace, SStoreTrace, SLoadTrace, Opcode
};
use crate::types::trace::{INSTRUCTIONS, VMTracer};
use super::vm_tracer::VmTracer;
use log::{warn, info};

pub trait ListenerVmTracer {
    fn step (&mut self, mes: &StepTrace);
    fn step_result (&mut self, mes: &StepResultTrace);
    fn sstore (&mut self, mes: &SStoreTrace);
    fn sload (&mut self, mes: &SLoadTrace);
}

impl ListenerVmTracer for VmTracer{
    fn step(&mut self, mes: &StepTrace){
        if let Some(pending_trap) = self.take_pending_trap() {
            self.handle_step_result(mes.stack, mes.memory, pending_trap.pushed);
        }

        let pc = mes.position.expect("trace position");
        // println!("pc = {:?} opcode = {:?}", pc, mes.opcode);
        let instruction = mes.opcode.0;
        let mem_written = mem_written(mes.opcode, mes.stack);
        let store_written = store_written(mes.opcode, mes.stack);
        self.tracer.trace_prepare_execute(
            pc,
            instruction,
            U256::from(0),
            mem_written,
            store_written.map(|(a, b)| (a, b)),
        );

        if let Some(pushed_count) = pushed(mes.opcode) {
            self.pushed = pushed_count;
        } else {
            warn!("{}", "Unknown opcode");
        }
    }

    fn step_result (&mut self, mes: &StepResultTrace){
        // println!("res");
        match mes.result {
            Ok(_) => self.handle_step_result(mes.stack, mes.memory, self.pushed),
            Err(err) => {
                match err {
                    Capture::Trap(opcode) => {
                        if matches!(*opcode, Opcode::SLOAD | Opcode::SSTORE) {
                            return; // Handled in separate events
                        }

                        let trap = PendingTrap {
                            pushed: self.pushed,
                            depth: self.tracer.depth,
                        };
                        self.trap_stack.push(trap);

                        match *opcode {
                            Opcode::CALL
                            | Opcode::CALLCODE
                            | Opcode::DELEGATECALL
                            | Opcode::STATICCALL => self.tracer.prepare_subtrace(&[]),
                            Opcode::LOG0
                            | Opcode::LOG1
                            | Opcode::LOG2
                            | Opcode::LOG3
                            | Opcode::LOG4 => {
                                VmTracer::handle_log(*opcode, mes.stack, mes.memory.data());
                            }
                            _ => (),
                        }

                        return;
                    }
                    Capture::Exit(err) => {
                        info!("exit with {:?}", err);
                        match err {
                            // RETURN, STOP as SUICIDE opcodes
                            ExitReason::Succeed(_success) => {
                                self.tracer.trace_executed(U256::zero(), &[], &[]);
                            }
                            ExitReason::Error(_)
                            | ExitReason::Fatal(_)
                            | ExitReason::Revert(_)
                            | ExitReason::StepLimitReached => self.tracer.trace_failed(),
                        }
                        self.tracer.done_subtrace();
                        if let Some(pending_trap) = self.take_pending_trap() {
                            self.handle_step_result(mes.stack, mes.memory, pending_trap.pushed);
                        }
                    }
                }
                self.pushed = 0;
            }
        }

    }

    fn sstore (&mut self, mes: &SStoreTrace){
        self.storage_accessed = Some((mes.index, mes.value));
        self.tracer.trace_executed(U256::zero(), &[], &[]);
        /* TODO */
    }

    fn sload (&mut self, mes: &SLoadTrace){
        self.storage_accessed = Some((mes.index, mes.value));
        self.tracer
            .trace_executed(U256::zero(), &[mes.value], &[]);
    }
}


pub fn pushed(opcode: Opcode) -> Option<usize> {
    INSTRUCTIONS
        .get(opcode.as_usize())
        .and_then(std::option::Option::as_ref)
        .map(|i| i.ret)
}

/// Checks whether offset and size is valid memory range
fn is_valid_range(off: usize, size: usize) -> bool {
    // When size is zero we haven't actually expanded the memory
    let overflow = off.overflowing_add(size).1;
    size > 0 && !overflow
}

#[allow(clippy::cast_possible_truncation)]
fn mem_written(instruction: Opcode, stack: &Stack) -> Option<(usize, usize)> {
    let read = |pos| stack.peek(pos).expect("stack peek error").low_u64() as usize;
    let written = match instruction {
        // Core codes
        Opcode::MSTORE | Opcode::MLOAD => Some((read(0), 32)),
        Opcode::MSTORE8 => Some((read(0), 1)),
        Opcode::CALLDATACOPY | Opcode::CODECOPY | Opcode::RETURNDATACOPY => Some((read(0), read(2))),
        // External codes
        Opcode::EXTCODECOPY => Some((read(1), read(3))),
        Opcode::CALL | Opcode::CALLCODE => Some((read(5), read(6))),
        Opcode::DELEGATECALL | Opcode::STATICCALL => Some((read(4), read(5))),
        /* Remaining external opcodes that do not affect memory:
          Opcode::SHA3 | Opcode::ADDRESS | Opcode::BALANCE | Opcode::SELFBALANCE | Opcode::ORIGIN
        | Opcode::CALLER | Opcode::CALLVALUE | Opcode::GASPRICE | Opcode::EXTCODESIZE
        | Opcode::EXTCODEHASH | Opcode::RETURNDATASIZE | Opcode::BLOCKHASH | Opcode::COINBASE
        | Opcode::TIMESTAMP | Opcode::NUMBER | Opcode::DIFFICULTY | Opcode::GASLIMIT
        | Opcode::CHAINID | Opcode::SLOAD | Opcode::SSTORE | Opcode::GAS | Opcode::LOG0
        | Opcode::LOG1 | Opcode::LOG2 | Opcode::LOG3 | Opcode::LOG4 | Opcode::CREATE
        | Opcode::CREATE2
        */
        _ => None,
    };


    match written {
        Some((offset, size)) if !is_valid_range(offset, size) => None,
        written => written,
    }
}

fn store_written(instruction: Opcode, stack: &Stack) -> Option<(U256, U256)> {
    match instruction {
        Opcode::SSTORE => Some((stack.peek(0).expect("stack.peek(0) error"), stack.peek(1).expect("stack.peek(1) error"))),
        _ => None,
    }
}

#[derive(Debug)]
pub struct PendingTrap {
    pub pushed: usize,
    pub depth: usize,
}



