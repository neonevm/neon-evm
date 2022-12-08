use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
};
use tokio_postgres::{ connect, Error, Client};
use postgres::{ NoTls};
use serde::{Serialize, Deserialize };
use tokio::task::block_in_place;
use std::convert::TryFrom;


#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DBConfig{
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug)]
pub struct PostgresClient {
    pub slot: u64,
    client: Client,
}

pub fn block<F, Fu, R>(f: F) -> R
    where
        F: FnOnce() -> Fu,
        Fu: std::future::Future<Output = R>,
{
    block_in_place(|| {
        let handle = tokio::runtime::Handle::current();
        handle.block_on(f())
    })
}

impl PostgresClient {
    pub fn new(config: &DBConfig, slot: u64) -> Self {
        let connection_str= format!("host={} port={} dbname={} user={} password={}",
                                    config.host, config.port, config.database, config.user, config.password);

        let mut attempt = 0;
        let mut result = None;

        while attempt < 3 {
            result = block(|| async {
                connect(&connection_str, NoTls).await
            }).ok();
            if result.is_some() {
                break;
            }
            attempt += 1;
        }

        let (client, connection) = result.expect("error to set DB connection");

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
        Self {slot, client}
    }

    pub fn get_account_at_slot(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let pubkey_bytes = pubkey.to_bytes();
        let rows = block(|| async {
            self.client.query(
                "SELECT * FROM get_account_at_slot($1, $2)",
                &[&pubkey_bytes.as_slice(), &(self.slot as i64)]
            ).await
        })?;

        if rows.len() != 1 {
            return Ok(None);
        }

        let row = &rows[0];
        let lamports: i64 = row.try_get(2)?;
        let rent_epoch: i64 = row.try_get(4)?;
        Ok(Some(Account {
            lamports: u64::try_from(lamports).expect("lamports parse error"),
            data: row.try_get(5)?,
            owner: Pubkey::new(row.try_get(1)?),
            executable: row.try_get(3)?,
            rent_epoch: u64::try_from(rent_epoch).expect("rent_epoch parse error"),
        }))
    }

    pub fn get_block_hash(&self, slot: u64) -> Result<String, Error> {
        let hash = block(|| async {
            self.client.query_one(
                "SELECT blockhash FROM public.block WHERE slot = $1",
                &[&(slot as i64)],
            ).await
        })?.try_get(0)?;

        Ok(hash)
    }

    pub fn get_block_time(&self, slot: u64) -> Result<i64, Error> {
        let time = block(|| async {
            self.client.query_one(
                "SELECT block_time FROM public.block WHERE slot = $1",
                &[&(slot as i64)],
            ).await
        })?.try_get(0)?;

        Ok(time)
    }

    pub fn get_latest_blockhash(&self) -> Result<String, Error> {
        let slot: i64 = block(|| async {
            self.client.query_one("SELECT MAX(slot) FROM public.slot", &[])
                .await
        })?.try_get(0)?;

        self.get_block_hash(u64::try_from(slot).expect("slot parse error"))
    }
}
