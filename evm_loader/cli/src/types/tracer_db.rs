use {
    std::{sync::Arc, convert::TryFrom},
    tokio_postgres::{Client},
    super::{do_connect, DbConfig, block, DbError, DbResult},
    solana_sdk::{pubkey::Pubkey, account::Account},
};


#[derive(Debug, Clone)]
pub struct TracerDb {
    pub client: Arc<Client>,
}

impl TracerDb {
    pub fn new(config: &DbConfig) -> Self {
        let client = do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        Self {client: Arc::new(client)}
    }

    pub fn get_block_hash(&self, slot: u64) -> DbResult<String>{
        let row = block(|| async {
            self.client.query_one(
                "SELECT blockhash FROM public.block WHERE slot = $1", &[&(slot as i64)],
            ).await
        })?;

        row.try_get(0).map_err(std::convert::Into::into)
    }

    pub fn get_block_time(&self, slot: u64) -> DbResult<i64> {
        let row = block(|| async {
            self.client.query_one(
                "SELECT block_time FROM public.block WHERE slot = $1", &[&(slot as i64)],
            ).await
        })?;

        row.try_get(0).map_err(std::convert::Into::into)
    }

    pub fn get_latest_block(&self) -> DbResult<u64>{
        let row = block(|| async {
            self.client.query_one("SELECT MAX(slot) FROM public.slot", &[]).await
        })?;

        let slot: i64 = row.try_get(0)?;
        u64::try_from(slot).map_err(|e| DbError::Custom(format!("slot cast error: {}", e)))
    }

    pub fn get_latest_blockhash(&self) -> DbResult<String>{
        self.get_block_hash(self.get_latest_block()?)
    }

    pub fn get_account_at(&self, pubkey: &Pubkey, slot: u64) -> DbResult<Option<Account>> {
        let pubkey_bytes = pubkey.to_bytes();
        let rows = block(|| async {
            self.client.query(
                "SELECT * FROM get_account_at_slot($1, $2)",
                &[&pubkey_bytes.as_slice(), &(slot as i64)]
            ).await
        })?;

        if rows.is_empty() { return Ok(None) }

        let row = &rows[0];
        let lamports: i64 = row.try_get(2)?;
        let rent_epoch: i64 = row.try_get(4)?;
        Ok(Some(Account {
            lamports: u64::try_from(lamports).map_err(|e| DbError::Custom(format!("lamports cast error: {}", e)))?,
            data: row.try_get(5)?,
            owner: Pubkey::new(row.try_get(1)?),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).map_err(|e| DbError::Custom(format!("rent_epoch cast error: {}", e)))?,
        }))
    }

    pub fn get_account_by_sol_sig(&self, pubkey: &Pubkey, sol_sig: &[u8; 64]) -> DbResult<Option<Account>> {
        let pubkey_bytes = pubkey.to_bytes();
        let row = block(|| async {
            self.client.query_one(
                "SELECT * FROM get_pre_accounts($1, $2)",
                &[&sol_sig.as_slice(), &[pubkey_bytes.as_slice()]]
            ).await
        })?;

        let lamports: i64 = row.try_get(0)?;
        let rent_epoch: i64 = row.try_get(4)?;

        let account = Account {
            lamports: u64::try_from(lamports).map_err(|e| DbError::Custom(format!("lamports cast error: {}", e)))?,
            data: row.try_get(1)?,
            owner: Pubkey::new(row.try_get(2)?),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).map_err(|e| DbError::Custom(format!("rent_epoch cast error: {}", e)))?,
        };

        Ok(Some(account))
    }
}