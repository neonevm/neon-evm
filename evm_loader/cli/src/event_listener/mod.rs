mod listener_tracer;
mod listener_vm_tracer;
mod vm_tracer;
pub mod tracer;

use evm_loader::{Event, EventListener};
use {
     tracer::Tracer,
     listener_tracer::ListenerTracer,
     listener_vm_tracer::ListenerVmTracer,
};

impl EventListener for Tracer{

    fn event(&mut self, event: Event){

        match event {
            Event::Step(trace) =>  {
                println!("Step: {:?}", trace);
                self.step(&trace);
                self.vm.step(&trace);
            },

            Event::StepResult(trace) =>  {
                println!("StepResult: {:?}", trace);
                self.vm.step_result(&trace);
            },

            Event::SLoad(trace) =>  {
                println!("SLoad: {:?}", trace);
                self.vm.sload(&trace);
            },

            Event::SStore(trace) => {
                println!("SStore: {:?}", trace);
                self.vm.sstore(&trace);
            },
        };
    }
}


