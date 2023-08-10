use std::sync::Arc;
use {
    super::{do_connect, ChDbConfig, PgError, PgResult, TxParams},
    ethnum::U256,
    evm_loader::types::Address,
    solana_sdk::clock::Slot,
    std::convert::{TryFrom, TryInto},
    tokio_postgres::Client,
};

#[derive(Debug, Clone)]
pub struct IndexerDb {
    pub client: Arc<Client>,
}

const TXPARAMS_FIELDS: &str =
    "from_addr, COALESCE(to_addr, contract), calldata, value, gas_limit, nonce";

impl IndexerDb {
    pub async fn new(config: &ChDbConfig) -> Self {
        let client = do_connect(
            &config.indexer_host,
            &config.indexer_port,
            &config.indexer_database,
            &config.indexer_user,
            &config.indexer_password,
        )
        .await;
        Self {
            client: Arc::new(client),
        }
    }

    pub async fn get_sol_sig(&self, hash: &[u8; 32]) -> PgResult<[u8; 64]> {
        let hex = format!("0x{}", hex::encode(hash));
        let row = self
            .client
            .query_one(
                "SELECT S.sol_sig from solana_neon_transactions S, solana_blocks B \
                where S.block_slot = B.block_slot \
                and B.is_active = true \
                and S.neon_sig = $1",
                &[&hex],
            )
            .await?;
        let sol_sig_b58: &str = row.try_get(0)?;
        let sol_sig_b58 = sol_sig_b58.to_string();
        let sol_sig = bs58::decode(sol_sig_b58)
            .into_vec()
            .map_err(|e| PgError::Custom(format!("sol_sig_b58 cast error: {e}")))?;
        sol_sig
            .as_slice()
            .try_into()
            .map_err(|e| PgError::Custom(format!("sol_sig cast error: {e}")))
    }

    pub async fn get_slot(&self, hash: &[u8; 32]) -> PgResult<Slot> {
        let hex = format!("0x{}", hex::encode(hash));
        let row = self
            .client
            .query_one(
                "SELECT min(S.block_slot) from solana_neon_transactions S, solana_blocks B \
                where S.block_slot = B.block_slot \
                and B.is_active = true \
                and S.neon_sig = $1",
                &[&hex],
            )
            .await?;
        let slot: i64 = row.try_get(0)?;
        u64::try_from(slot).map_err(|e| PgError::Custom(format!("slot cast error: {e}")))
    }

    #[allow(unused)]
    pub async fn get_slot_by_block_hash(&self, block_hash: &[u8; 32]) -> PgResult<u64> {
        let hex = format!("0x{}", hex::encode(block_hash));
        let row = self
            .client
            .query_one(
                "SELECT block_slot FROM solana_blocks WHERE block_hash = $1 AND is_active = TRUE",
                &[&hex],
            )
            .await?;

        let slot: i64 = row
            .try_get(0)
            .map_err(std::convert::Into::<PgError>::into)?;

        slot.try_into()
            .map_err(|e| PgError::Custom(format!("slot cast error: {e}")))
    }

    pub async fn get_transaction_data(&self, hash: &[u8; 32]) -> PgResult<TxParams> {
        let hex = format!("0x{}", hex::encode(hash));

        let row = self
            .client
            .query_one(
                &format!(
                    "select distinct {TXPARAMS_FIELDS} \
                    from neon_transactions as t, solana_blocks as b \
                    where t.block_slot = b.block_slot \
                        and b.is_active = TRUE \
                        and t.neon_sig = $1"
                ),
                &[&hex],
            )
            .await?;

        Self::extract_transaction(&row)
    }

    pub async fn get_block_transactions(&self, slot: u64) -> PgResult<Vec<TxParams>> {
        let slot: i64 = slot
            .try_into()
            .map_err(|e| PgError::Custom(format!("slot cast error: {e}")))?;

        let rows = self
            .client
            .query(
                &format!(
                    "\
                SELECT {TXPARAMS_FIELDS} \
                FROM neon_transactions t \
                    INNER JOIN solana_blocks b ON t.block_slot = b.block_slot \
                WHERE b.is_active = TRUE AND t.block_slot = $1 \
                ORDER BY tx_idx\
            "
                ),
                &[&slot],
            )
            .await?;

        let mut transactions = vec![];
        for row in rows {
            transactions.push(Self::extract_transaction(&row)?);
        }

        Ok(transactions)
    }

    fn extract_transaction(row: &tokio_postgres::Row) -> PgResult<TxParams> {
        let from: String = row.try_get(0)?;
        let to: String = row.try_get(1)?;
        let data: String = row.try_get(2)?;
        let value: String = row.try_get(3)?;
        let gas_limit: String = row.try_get(4)?;
        let nonce: String = row.try_get(5)?;

        let from = Address::from_hex(&from.as_str()[2..])
            .map_err(|e| PgError::Custom(format!("from_address cast error: {e}")))?;
        let to = Address::from_hex(&to.as_str()[2..])
            .map_err(|e| PgError::Custom(format!("to_address cast error: {e}")))?;
        let data = hex::decode(&data.as_str()[2..])
            .map_err(|e| PgError::Custom(format!("data cast error: {e}")))?;
        let value: U256 = U256::from_str_hex(&value)
            .map_err(|e| PgError::Custom(format!("value cast error: {e}")))?;
        let gas_limit: U256 = U256::from_str_hex(&gas_limit)
            .map_err(|e| PgError::Custom(format!("gas_limit cast error: {e}")))?;
        let nonce: u64 = U256::from_str_hex(&nonce)
            .map_err(|e| PgError::Custom(format!("nonce cast error: {e}")))?
            .try_into()
            .map_err(|e| PgError::Custom(format!("nonce cast error: {e}")))?;

        Ok(TxParams {
            nonce: Some(nonce),
            from,
            to: Some(to),
            data: Some(data),
            value: Some(value),
            gas_limit: Some(gas_limit),
        })
    }
}
