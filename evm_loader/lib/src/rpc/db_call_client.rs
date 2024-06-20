use super::{e, Rpc};
use crate::types::TracerDb;
use crate::NeonError;
use async_trait::async_trait;
use log::debug;
use solana_client::{
    client_error::Result as ClientResult,
    client_error::{ClientError, ClientErrorKind},
};
use solana_sdk::{
    account::Account,
    clock::{Slot, UnixTimestamp},
    pubkey::Pubkey,
};

pub struct CallDbClient {
    tracer_db: TracerDb,
    slot: u64,
    tx_index_in_block: Option<u64>,
}

impl CallDbClient {
    pub async fn new(
        tracer_db: TracerDb,
        slot: u64,
        tx_index_in_block: Option<u64>,
    ) -> Result<Self, NeonError> {
        let earliest_rooted_slot = tracer_db
            .get_earliest_rooted_slot()
            .await
            .map_err(NeonError::ClickHouse)?;
        if slot < earliest_rooted_slot {
            return Err(NeonError::EarlySlot(slot, earliest_rooted_slot));
        }

        Ok(Self {
            tracer_db,
            slot,
            tx_index_in_block,
        })
    }

    async fn get_account_at(&self, key: &Pubkey) -> ClientResult<Option<Account>> {
        self.tracer_db
            .get_account_at(key, self.slot, self.tx_index_in_block)
            .await
            .map_err(|e| e!("load account error", key, e))
    }
}

#[async_trait(?Send)]
impl Rpc for CallDbClient {
    async fn get_account(&self, key: &Pubkey) -> ClientResult<Option<Account>> {
        self.get_account_at(key).await
    }

    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> ClientResult<Vec<Option<Account>>> {
        let mut result = Vec::new();
        for key in pubkeys {
            result.push(self.get_account_at(key).await?);
        }
        debug!("get_multiple_accounts: pubkeys={pubkeys:?} result={result:?}");
        Ok(result)
    }

    async fn get_block_time(&self, slot: Slot) -> ClientResult<UnixTimestamp> {
        self.tracer_db
            .get_block_time(slot)
            .await
            .map_err(|e| e!("get_block_time error", slot, e))
    }

    async fn get_slot(&self) -> ClientResult<Slot> {
        Ok(self.slot)
    }

    async fn get_deactivated_solana_features(&self) -> ClientResult<Vec<Pubkey>> {
        Ok(vec![]) // TODO
    }
}
