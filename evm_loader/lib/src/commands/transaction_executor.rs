use std::cell::RefCell;
use std::future::Future;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use {
    crate::{errors::NeonError, rpc},
    log::{debug, error, info, warn},
    solana_sdk::{
        account::Account,
        commitment_config::CommitmentConfig,
        instruction::Instruction,
        program_pack::{IsInitialized, Pack},
        pubkey::Pubkey,
        signature::Signature,
        signer::Signer,
        signers::Signers,
        transaction::Transaction,
    },
};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Stats {
    pub total_objects: u32,
    pub corrected_objects: u32,
    pub invalid_objects: u32,
    pub modified_objects: u32,
    pub created_objects: u32,
}
impl Stats {
    pub fn inc_corrected_objects(&mut self) {
        self.total_objects += 1;
        self.corrected_objects += 1;
    }
    pub fn inc_invalid_objects(&mut self) {
        self.total_objects += 1;
        self.invalid_objects += 1;
    }
    pub fn inc_modified_objects(&mut self) {
        self.total_objects += 1;
        self.modified_objects += 1;
    }
    pub fn inc_created_objects(&mut self) {
        self.total_objects += 1;
        self.created_objects += 1;
    }
}
pub struct TransactionExecutor<'a, 'b> {
    pub client: &'a dyn rpc::Rpc,
    pub send_trx: bool,
    pub signatures: RwLock<Vec<Signature>>,
    pub stats: RefCell<Stats>,
    pub fee_payer: &'b dyn Signer,
}

impl<'a, 'b> TransactionExecutor<'a, 'b> {
    pub fn new(client: &'a dyn rpc::Rpc, fee_payer: &'b dyn Signer, send_trx: bool) -> Self {
        Self {
            client,
            send_trx,
            signatures: RwLock::new(vec![]),
            stats: RefCell::new(Stats::default()),
            fee_payer,
        }
    }

    pub async fn get_account(&self, account_key: &Pubkey) -> Result<Option<Account>, NeonError> {
        let account_info = self
            .client
            .get_account_with_commitment(account_key, self.client.commitment())
            .await?
            .value;
        Ok(account_info)
    }

    pub async fn get_account_data_pack<T: Pack + IsInitialized>(
        &self,
        owner_program_id: &Pubkey,
        account_key: &Pubkey,
    ) -> Result<Option<T>, NeonError> {
        if let Some(account_info) = self.get_account(account_key).await? {
            if account_info.data.is_empty() {
                return Err(NeonError::AccountNotFound(*account_key));
            }
            if account_info.owner != *owner_program_id {
                return Err(NeonError::IncorrectProgram(account_info.owner));
            }

            let account: T = T::unpack(&account_info.data)?;
            if !account.is_initialized() {
                return Err(NeonError::AccountNotFound(*account_key));
            }
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }

    pub async fn checkpoint(&self, commitment: CommitmentConfig) -> Result<(), NeonError> {
        let recent_blockhash = self.client.get_latest_blockhash().await?;
        for sig in self.signatures.read().await.iter() {
            self.client
                .confirm_transaction_with_spinner(sig, &recent_blockhash, commitment)
                .await?;
        }
        Ok(())
    }

    pub async fn create_transaction<T: Signers>(
        &self,
        instructions: &[Instruction],
        signing_keypairs: &T,
    ) -> Result<Transaction, NeonError> {
        let mut transaction =
            Transaction::new_with_payer(instructions, Some(&self.fee_payer.pubkey()));

        let blockhash = self.client.get_latest_blockhash().await?;
        transaction.try_partial_sign(&[self.fee_payer], blockhash)?;
        transaction.try_sign(signing_keypairs, blockhash)?;

        Ok(transaction)
    }

    pub async fn create_transaction_with_payer_only(
        &self,
        instructions: &[Instruction],
    ) -> Result<Transaction, NeonError> {
        self.create_transaction::<[&dyn Signer; 0]>(instructions, &[])
            .await
    }

    pub async fn send_transaction(
        &self,
        transaction: &Transaction,
    ) -> Result<Signature, NeonError> {
        self.client
            .send_transaction(transaction)
            .await
            .map_err(std::convert::Into::into)
    }

    pub async fn check_and_create_object<T, V, C, Fu1, Fu2>(
        &self,
        name: &str,
        object: Result<Option<T>, NeonError>,
        verify: V,
        create: C,
    ) -> Result<Option<Signature>, NeonError>
    where
        Fu1: Future<Output = Result<Option<Transaction>, NeonError>>,
        Fu2: Future<Output = Result<Option<Transaction>, NeonError>>,
        V: FnOnce(T) -> Fu2,
        C: FnOnce() -> Fu1,
        T: std::fmt::Debug + Clone,
    {
        if let Some(data) = object.map_err(|e| {
            error!("{}: {:?}", name, e);
            e
        })? {
            debug!("{}: {:?}", name, data);
            match verify(data.clone()).await {
                Ok(None) => {
                    info!("{}: correct", name);
                    self.stats.borrow_mut().inc_corrected_objects();
                }
                Ok(Some(transaction)) => {
                    if self.send_trx {
                        let result = self.send_transaction(&transaction).await;
                        match result {
                            Ok(signature) => {
                                warn!("{}: updated in trx {}", name, signature);
                                self.signatures.write().await.push(signature);
                                self.stats.borrow_mut().inc_modified_objects();
                                return Ok(Some(signature));
                            }
                            Err(error) => {
                                error!("{}: failed update with {}", name, error);
                                self.stats.borrow_mut().inc_invalid_objects();
                                return Err(error);
                            }
                        };
                    };
                    debug!("{}: {:?}", name, transaction);
                    self.stats.borrow_mut().inc_invalid_objects();
                    warn!("{}: will be updated", name);
                }
                Err(error) => {
                    error!("{}: wrong object {:?}", name, error);
                    self.stats.borrow_mut().inc_invalid_objects();
                    if self.send_trx {
                        return Err(error);
                    }
                }
            }
        } else {
            match create().await {
                Ok(None) => {
                    info!("{}: missed ok", name);
                    self.stats.borrow_mut().inc_corrected_objects();
                }
                Ok(Some(transaction)) => {
                    if self.send_trx {
                        let result = self.send_transaction(&transaction).await;
                        match result {
                            Ok(signature) => {
                                warn!("{}: created in trx {}", name, signature);
                                self.signatures.write().await.push(signature);
                                self.stats.borrow_mut().inc_created_objects();
                                return Ok(Some(signature));
                            }
                            Err(error) => {
                                error!("{}: failed create with {}", name, error);
                                self.stats.borrow_mut().inc_invalid_objects();
                                return Err(error);
                            }
                        };
                    };
                    debug!("{}: {:?}", name, transaction);
                    warn!("{}: will be created", name);
                    self.stats.borrow_mut().inc_created_objects();
                }
                Err(error) => {
                    error!("{}: can't be created: {:?}", name, error);
                    self.stats.borrow_mut().inc_invalid_objects();
                    if self.send_trx {
                        return Err(error);
                    }
                }
            }
        }
        Ok(None)
    }
}
