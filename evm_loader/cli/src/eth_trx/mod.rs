mod base;

use solana_sdk::{message::{Message, SanitizedMessage}};
use crate::{types::TxMeta, config::Config};


pub struct EthTrx<'a> {
    pub message: SanitizedMessage,
    pub meta: TxMeta<()>,
    pub config: &'a Config,
}


impl<'a> ToEthereumTransaction for EthTrx<'a>{
    fn to_solana_trx(&mut self) -> Result<SolanaTransaction, anyhow::Error> {
        self.load_accounts()?;

        Ok(
            SolanaTransaction{
                accounts: self.accounts.clone(),  //TODO it is not well..
                messsage: self.message.clone()
            }
        )
    }

    fn program_id(&self) -> &Pubkey {
        &self.config.evm_loader
    }

    fn slot(&self) ->  u64{ self.meta.slot }
}