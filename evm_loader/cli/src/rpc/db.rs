use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
};
use tokio_postgres::{ connect, Error, Client};
use postgres::{ NoTls};
use serde::{Serialize, Deserialize };
use tokio::task::block_in_place;
use std::convert::TryFrom;
use evm_loader::H256;


#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DbConfig{
    pub tracer_host: String,
    pub tracer_port: String,
    pub tracer_database: String,
    pub tracer_user: String,
    pub tracer_password: String,
    pub indexer_host: String,
    pub indexer_port: String,
    pub indexer_database: String,
    pub indexer_user: String,
    pub indexer_password: String,
}


#[derive(Debug)]
pub struct CallDbClient {
    pub slot: u64,
    tracer_db: Client,
}

#[derive(Debug)]
pub struct TrxDbClient {
    pub hash: H256,
    tracer_db: Client,
    indexer_db: Client,
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
    pub fn new_for_eth_call(config: &DbConfig, slot: u64) -> Self {
        let client = PostgresClient::do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        PostgresClient {
            slot: Some(slot),
            hash: None,
            tracer_client : client,
            indexer_client: None,
        }
    }

    pub fn new_for_trx(config: &DBconfig, hash: H256) -> Self {
        let tracer_client = PostgresClient::do_connect(
            &config.tracer_host, &config.tracer_port, &config.tracer_database, &config.tracer_user, &config.tracer_password
        );
        let indexer_client = PostgresClient::do_connect(
            &config.indexer_host, &config.indexer_port, &config.indexer_database, &config.indexer_user, &config.indexer_password
        );
        PostgresClient {
            slot: None,
            hash: Some(hash),
            tracer_client : tracer_client,
            indexer_client: Some(indexer_client),
        }
    }


    pub fn do_connect(host: &String, port: &String, db: &String, user: &String, pass: &String) -> Client {
        let authority= format!(
            "host={} port={} dbname={} user={} password={}", host, port, db, user, pass
        );

        let mut attempt = 0;
        let mut result = None;

        while attempt < 3 {
            result = block(|| async {
                connect(&authority, NoTls).await
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
        client
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

    pub fn get_transaction_data(&self, hash: &String) -> Result<> {
        let row = block(|| async {
            self.client.query(
                "select distinct from_addr, to_addr, calldata, value, gas_used, gas_limit\
                 from neon_transactions where neon_sig = {}", hash
            ).await;
        })?;


    }
}
