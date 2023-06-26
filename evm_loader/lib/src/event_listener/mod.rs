mod listener_tracer;
mod listener_vm_tracer;
pub mod tracer;
mod vm_tracer;

use evm_loader::evm::tracing::{Event, EventListener};
use {listener_tracer::ListenerTracer, listener_vm_tracer::ListenerVmTracer, tracer::Tracer};

impl EventListener for Tracer {
    fn event(&mut self, event: Event) {
        match event {
            Event::BeginVM { context, code } => {
                self.vm.begin_vm(context, code);
            }
            Event::EndVM { status } => {
                self.vm.end_vm(status);
            }
            Event::BeginStep {
                opcode,
                pc,
                stack,
                memory,
            } => {
                self.begin_step(stack, memory);
                self.vm.begin_step(opcode, pc);
            }
            Event::EndStep { gas_used } => {
                self.end_step();
                self.vm.end_step(gas_used);
            }
            Event::StackPush { value } => {
                self.vm.stack_push(value);
            }
            Event::MemorySet { offset, data } => {
                self.vm.memory_set(offset, data);
            }
            Event::StorageSet { index, value } => {
                self.vm.storage_set(index, value);
            }
            Event::StorageAccess { index, value } => {
                self.vm.storage_access(index, value);
            }
        };
    }
}
