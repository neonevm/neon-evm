use log::{warn, info};
use crate::types::trace::{ExecutiveVMTracer, VMTracer};
use evm_loader::{Memory, U256, Opcode, Stack};
use super::listener_vm_tracer::PendingTrap;

pub struct VmTracer {
    pub tracer: ExecutiveVMTracer,
    pub pushed: usize,
    gas: u64,
    pub storage_accessed: Option<(U256, U256)>,
    pub trap_stack: Vec<PendingTrap>,
}

impl VmTracer {
    pub fn init() -> Self {
        let mut tracer = ExecutiveVMTracer::toplevel();
        tracer.prepare_subtrace(&[]);

        VmTracer {
            tracer,
            pushed: 0,
            gas: 0,
            storage_accessed: None,
            trap_stack: Vec::new(),
        }
    }

    pub fn handle_log(opcode: Opcode, stack: &Stack, memory: &[u8]) {
        info!("handling log {:?}", opcode);
        let offset = stack.peek(0).ok();
        let length = stack.peek(1).ok();
        let mut topics = Vec::new();
        match opcode {
            Opcode::LOG0 => {}
            Opcode::LOG1 => {
                topics.push(stack.peek(2));
            }
            Opcode::LOG2 => {
                topics.push(stack.peek(2));
                topics.push(stack.peek(3));
            }
            Opcode::LOG3 => {
                topics.push(stack.peek(2));
                topics.push(stack.peek(3));
                topics.push(stack.peek(4));
            }
            Opcode::LOG4 => {
                topics.push(stack.peek(2));
                topics.push(stack.peek(3));
                topics.push(stack.peek(4));
                topics.push(stack.peek(5));
            }
            _ => warn!("unexpected log opcode: {:?}", opcode),
        }

        if let (Some(offset), Some(length)) = (offset, length) {
            //let offset: ethereum_types::H256 = offset.to();
            let offset = offset.as_usize();
            let length = length.as_usize();
            info!(
                "evm event {:?} @ ({}, {})",
                memory.get(offset..offset + length),
                offset,
                offset + length
            );
        }
    }

    pub fn take_pending_trap(&mut self) -> Option<PendingTrap> {
        if self.trap_stack.last()?.depth == self.tracer.depth {
            self.trap_stack.pop()
        } else {
            None
        }
    }

    pub fn handle_step_result(&mut self, stack: &Stack, mem: &Memory, pushed: usize) {
        let gas_used = U256::from(self.gas);
        let mut stack_push = vec![];
        for i in (0..pushed).rev() {
            stack_push.push(stack.peek(i).expect("stack peek error"));
        }
        let mem = &mem.data();
        self.tracer.trace_executed(gas_used, &stack_push, mem);
    }
}
