use solana_sdk::{
    account::Account,
    pubkey::Pubkey,
};
use tokio_postgres::{ connect, Error};
use postgres::{ NoTls };
use serde::{Serialize, Deserialize };

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DBConfig{
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
}


pub struct PostgresClient {
    // client: DBClient,
    config: DBConfig,
    pub slot: u64,
}


impl PostgresClient {
    // TODO database connect must be only once
    pub  fn new(config: &DBConfig, slot: u64) -> Self {
        Self { config: config.clone() , slot}
    }

    // TODO: remove connection init
    #[tokio::main]
    pub async fn get_accounts_at_slot(&self, keys: impl Iterator<Item = Pubkey>) -> Result<Vec<(Pubkey, Account)>, Error> {

        let connection_str= format!("host={} port={} dbname={} user={} password={}",
                                    self.config.host, self.config.port, self.config.database, self.config.user, self.config.password);
        let (client, connection) = connect(&connection_str, NoTls).await.unwrap();

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        let key_bytes = keys.map(|entry| entry.to_bytes()).collect::<Vec<_>>();
        let key_slices = key_bytes.iter().map(|entry| entry.as_slice()).collect::<Vec<_>>();

        let rows = client.query(
            "SELECT * FROM get_accounts_at_slot($1, $2)",&[&key_slices, &(self.slot as i64)]
        ).await?;

        let mut result = vec![];
        for row in rows {
            let lamports: i64 = row.try_get(2)?;
            let rent_epoch: i64 = row.try_get(4)?;
            result.push((
                Pubkey::new(row.try_get(0)?),
                Account {
                    lamports: lamports as u64,
                    data: row.try_get(5)?,
                    owner: Pubkey::new(row.try_get(1)?),
                    executable: row.try_get(3)?,
                    rent_epoch: rent_epoch as u64,
                }
            ));
        }
        Ok(result)
    }

    pub fn get_account_at_slot(&self, pubkey: &Pubkey) -> Result<Option<Account>, Error> {
        let accounts = self.get_accounts_at_slot(std::iter::once(*pubkey))?;
        let account = accounts.get(0).map(|(_, account)| account).cloned();
        Ok(account)
    }
}
