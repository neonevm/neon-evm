use super::Player;
use crate::{service::{Config, TracedCall}, account_state::prepare_state_diff,
            types::{TxMeta, ec::transaction::{Action, SignedTransaction, TypedTransaction}},
            event_listener::tracer::Tracer, eth_trx::EthTrx, emulator::{Emulator, ToSolanaTransaction},
            v1::types::EthCallObject, eth_call::EthCall};
use anyhow;
use evm_loader::{H256, H160, ExitReason};
use solana_sdk::{message::{Message, SanitizedMessage}, };
use solana_bpf_loader_program::syscalls as syscalls;


impl<'a> Player<'a> {
    pub fn new(config: &'a Config, trace_code: Option<String>) -> Self {
        Self{ config: config, trace_code : trace_code }
    }

    pub fn replay_trx_hash(&self, hash: H256) -> Result<Vec<TxMeta<TracedCall>>, anyhow::Error>{
        let tx_metas = self.config.rpc_client.get_transaction_data(hash)?;

        self.config.rpc_client.get_transaction_data(hash)?.into_iter()
            .map(|tx_meta | self.replay_trx(tx_meta)).collect()
    }

    pub fn replay_trx_block(&self, slot: u64) -> Result<Vec<TxMeta<TracedCall>>, anyhow::Error> {
        self.config.rpc_client.get_transactions_by_slot(slot)?
            .into_iter().map(|tx_meta| self.replay_trx(tx_meta)).collect()
    }

    pub fn replay_trx_filter(
        &self,
        from_slot: Option<u64>,
        to_slot: Option<u64>,
        from: Option<Vec<H160>>,
        to: Option<Vec<H160>>,
        offset: Option<usize>,
        count: Option<usize>,
    ) -> Result<Vec<TxMeta<TracedCall>>, anyhow::Error> {
        let tx_metas = self.config.rpc_client.get_transactions(
            from_slot, to_slot, from, to, offset, count,
        )?;

        Err(anyhow::anyhow!("need to implement"))
        // tx_metas.into_iter().map(|tx_meta| self.replay_trx(tx_meta)).collect()
    }

    pub fn replay_raw_eth_call(&self, transaction: &Vec<u8>, block: u64) -> Result<TracedCall, anyhow::Error> {

        let tx = TypedTransaction::decode(transaction)?;
        let tx = SignedTransaction::new(tx)?;

        let mut eth_call = EthCallObject {
            from: Some(tx.sender().into()),
            to: match tx.unsigned.tx().action {
                Action::Call(addr) => Some(addr.into()),
                _ => None
            },
            data: Some(tx.unsigned.tx().data.clone().into()),
            value: Some(tx.unsigned.tx().gas.into()),
            gas: Some(tx.unsigned.tx().gas.into()),
            gasprice: None,
        };

        self.replay_eth_call(&eth_call, block)
    }

    pub fn replay_eth_call(&self, object: &EthCallObject, block: u64) -> Result<TracedCall, anyhow::Error> {
        let mut eth_call = EthCall::new(object, block, &self.config );
        self.replay(&mut eth_call)
    }

    fn replay_trx(&self, tx_meta: TxMeta<SanitizedMessage>) -> Result<TxMeta<TracedCall>, anyhow::Error>{
        let mut trx = EthTrx::new(tx_meta.clone(), self.config );
        let (mut meta, _) = tx_meta.split();
        let traced_call = self.replay( &mut trx)?;
        Ok(meta.wrap(traced_call))
    }

    fn replay<T>(&self,  transaction: &mut T) -> Result<TracedCall, anyhow::Error>
    where T: ToSolanaTransaction
    {

        let js_tracer = self.trace_code
            .as_ref()
            .map(|code| crate::js::JsTracer::new(code).unwrap())
            .map(|tracer| Box::new(tracer) as Box<_>);

        let mut tracer = Tracer::new(js_tracer);

        // let mut emulator = Emulator::new(transaction, &mut tracer )?;
        let mut emulator = Emulator::new(transaction )?;
        emulator.process( &mut tracer);

        let state_diff = prepare_state_diff(self.config, &tracer, transaction.slot());
        let (vm_trace, traces, full_trace_data, js_trace, result) = tracer.into_traces();

        Ok(TracedCall{
            vm_trace,
            state_diff: state_diff,
            traces,
            full_trace_data,
            js_trace,
            result,
            used_gas: 0,
            exit_reason: ExitReason::StepLimitReached,  // TODO add event ?
        })
    }
}
