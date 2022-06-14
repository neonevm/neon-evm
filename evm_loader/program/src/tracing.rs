// use evm::tracing::Event;
// use solana_program::{tracer_api, program_error::ProgramError};
// use solana_program::{compute_meter_remaining, compute_meter_set_remaining};
//
// pub fn send(event: &Event){
//     let mut remaining : u64 =0;
//     compute_meter_remaining::compute_meter_remaining(&mut remaining);
//
//     let mut message = vec![];
//     bincode::serialize_into(&mut message, event).map_err(|e| E!(ProgramError::InvalidInstructionData; "Error={:?}", e)).unwrap();
//     tracer_api::send_trace_message(message.as_slice());
//     // solana_program::msg!("{}", remaining);
//     compute_meter_set_remaining::compute_meter_set_remaining(remaining+12);
// }
