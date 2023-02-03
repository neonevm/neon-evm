use {
    std::{sync::Arc, convert::TryFrom},
    tokio_postgres::{Client},
    super::{do_connect, DbConfig, block, f},
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

    #[allow(unused)]
    pub fn new_from_client(c: Arc<Client>) -> Self { Self { client: c}}

    pub fn get_block_hash(&self, slot: u64) -> Result<String, String>{
        let row = block(|| async {
            self.client.query_one(
                "SELECT blockhash FROM public.block WHERE slot = $1", &[&(slot as i64)],
            ).await
        }).map_err(|e| f!(e))?;

        row.try_get(0).map_err(|e| f!(e))
    }

    pub fn get_block_time(&self, slot: u64) -> Result<i64, String>{
        let row = block(|| async {
            self.client.query_one(
                "SELECT block_time FROM public.block WHERE slot = $1", &[&(slot as i64)],
            ).await
        }).map_err(|e| f!(e))?;

        row.try_get(0).map_err(|e| f!(e))
    }

    pub fn get_latest_block(&self) -> Result<u64, String>{
        let row = block(|| async {
            self.client.query_one("SELECT MAX(slot) FROM public.slot", &[]).await
        }).map_err(|e| f!(e))?;

        let slot: i64 = row.try_get(0).map_err(|e| f!(e))?;
        u64::try_from(slot).map_err(|e| f!(e))
    }

    pub fn get_latest_blockhash(&self) -> Result<String, String>{
        self.get_block_hash(self.get_latest_block()?)
    }

    pub fn get_account_at(&self, pubkey: &Pubkey, slot: u64) -> Result<Option<Account>, String> {
        let pubkey_bytes = pubkey.to_bytes();
        let rows = block(|| async {
            self.client.query(
                "SELECT * FROM get_account_at_slot($1, $2)",
                &[&pubkey_bytes.as_slice(), &(slot as i64)]
            ).await
        }).map_err(|e| f!(e))?;

        if rows.is_empty() { return Ok(None) }

        let row = &rows[0];
        let lamports: i64 = row.try_get(2).map_err(|e| f!(e))?;
        let rent_epoch: i64 = row.try_get(4).map_err(|e| f!(e))?;
        Ok(Some(Account {
            lamports: u64::try_from(lamports).map_err(|e| f!(e))?,
            data: row.try_get(5).map_err(|e| f!(e))?,
            owner: Pubkey::new(row.try_get(1).map_err(|e| f!(e))?),
            executable: row.try_get(3).map_err(|e| f!(e))?,
            rent_epoch: u64::try_from(rent_epoch).map_err(|e| f!(e))?,
        }))
    }

    pub fn get_account_by_sol_sig(&self, pubkey: &Pubkey, sol_sig: &[u8; 64]) -> Result<Option<Account>, String> {
        let pubkey_bytes = pubkey.to_bytes();
        let row = block(|| async {
            self.client.query_one(
                "SELECT * FROM get_pre_accounts($1, $2)",
                &[&sol_sig.as_slice(), &[pubkey_bytes.as_slice()]]
            ).await
        }).map_err(|e| f!(e))?;

        let lamports: i64 = row.try_get(0).map_err(|e| f!(e))?;
        let rent_epoch: i64 = row.try_get(4).map_err(|e| f!(e))?;

        let account = Account {
            lamports: u64::try_from(lamports).map_err(|e| f!(e))?,
            data: row.try_get(1).map_err(|e| f!(e))?,
            owner: Pubkey::new(row.try_get(2).map_err(|e| f!(e))?),
            executable: row.try_get(3).map_err(|e| f!(e))?,
            rent_epoch: u64::try_from(rent_epoch).map_err(|e| f!(e))?,
        };

        Ok(Some(account))
    }
}