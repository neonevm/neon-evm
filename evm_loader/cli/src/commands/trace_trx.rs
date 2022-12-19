use crate::{Config, NeonCliResult, event_listener::tracer::Tracer, commands::{trace_call, TxParams,},
            types::ec::{trace::{FullTraceData, VMTrace},}, rpc::Rpc,
};
use evm_loader::{ExitReason, H256};
use super::parse_token_chain_steps;

pub fn execute(config: &Config, hash: H256) -> NeonCliResult {

    let (token, chain, max_steps) = parse_token_chain_steps(config, params);

    let tx = config.rpc_client.get_transaction_data(hash)?;

    let tx = TxParams {
        from: tx.from,
        to: tx.to,
        data: tx.data,
        value: tx.value,
        token,
        chain,
        max_steps,
        gas_limit
    };
    trace_call::execute(config, &tx)
}


