use serde::{Serialize, Deserialize};

use {
    log::{error, warn, info, debug},
    std::cell::RefCell,
    solana_sdk::{
        transaction::Transaction,
        instruction::Instruction,
        signers::Signers,
        program_pack::{IsInitialized, Pack},
        account::Account,
        pubkey::Pubkey,
        signer::Signer,
        signature::Signature,
        commitment_config::CommitmentConfig,
    },
    crate::{errors::NeonCliError, rpc}
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
    pub fn inc_corrected_objects(&mut self) {self.total_objects += 1; self.corrected_objects += 1;}
    pub fn inc_invalid_objects(&mut self) {self.total_objects += 1; self.invalid_objects += 1;}
    pub fn inc_modified_objects(&mut self) {self.total_objects += 1; self.modified_objects += 1;}
    pub fn inc_created_objects(&mut self) {self.total_objects += 1; self.created_objects += 1;}
}
pub struct TransactionExecutor<'a> {
    pub client: &'a dyn rpc::Rpc,
    pub send_trx: bool,
    pub signatures: RefCell<Vec<Signature>>,
    pub stats: RefCell<Stats>,
    pub fee_payer: &'a dyn Signer,
}

impl<'a> TransactionExecutor<'a> {
    pub fn new(client: &'a dyn rpc::Rpc, fee_payer: &'a dyn Signer, send_trx: bool) -> Self {
        Self {
            client,
            send_trx,
            signatures: RefCell::new(vec!()),
            stats: RefCell::new(Stats::default()),
            fee_payer,
        }
    }

    pub fn get_account(&self, account_key: &Pubkey) -> Result<Option<Account>,NeonCliError> {
        let account_info = self.client.get_account_with_commitment(
                account_key, self.client.commitment())?.value;
        Ok(account_info)
    }

    pub fn get_account_data_pack<T: Pack + IsInitialized>(
            &self,
            owner_program_id: &Pubkey,
            account_key: &Pubkey,
    ) -> Result<Option<T>,NeonCliError> {
        if let Some(account_info) = self.get_account(account_key)? {
            if account_info.data.is_empty() {
                return Err(NeonCliError::AccountNotFound(*account_key));
            }
            if account_info.owner != *owner_program_id {
                return Err(NeonCliError::IncorrectProgram(account_info.owner));
            }
    
            let account: T = T::unpack(&account_info.data)?;
            if !account.is_initialized() {
                return Err(NeonCliError::AccountNotFound(*account_key));
            }
            Ok(Some(account))
        } else {
            Ok(None)
        }
    }

    pub fn checkpoint(&self, commitment: CommitmentConfig) -> Result<(),NeonCliError> {
        let recent_blockhash = self.client.get_latest_blockhash()?;
        for sig in self.signatures.borrow().iter() {
            self.client.confirm_transaction_with_spinner(sig, &recent_blockhash, commitment)?;
        };
        Ok(())
    }

    pub fn create_transaction<T: Signers>(
            &self,
            instructions: &[Instruction],
            signing_keypairs: &T,
    ) -> Result<Transaction,NeonCliError> {
        let mut transaction = Transaction::new_with_payer(instructions, Some(&self.fee_payer.pubkey()));

        let blockhash = self.client.get_latest_blockhash()?;
        transaction.try_partial_sign(&[self.fee_payer], blockhash)?; 
        transaction.try_sign(signing_keypairs, blockhash)?;

        Ok(transaction)
    }

    pub fn create_transaction_with_payer_only(
            &self,
            instructions: &[Instruction]
    ) -> Result<Transaction,NeonCliError> {
        self.create_transaction::<[&dyn Signer; 0]>(instructions, &[])
    }

    pub fn send_transaction(&self, transaction: &Transaction) -> Result<Signature,NeonCliError> {
        self.client.send_transaction(transaction).map_err(std::convert::Into::into)
    }

    pub fn check_and_create_object<T,V,C>(&self, name: &str,
            object: Result<Option<T>,NeonCliError>, verify: V, create: C) -> Result<Option<Signature>,NeonCliError>
    where
        V: FnOnce(&T) -> Result<Option<Transaction>,NeonCliError>,
        C: FnOnce() -> Result<Option<Transaction>,NeonCliError>,
        T: std::fmt::Debug,
    {
        if let Some(data) = object.map_err(|e| {error!("{}: {:?}", name, e); e})? {
            debug!("{}: {:?}", name, data);
            match verify(&data) {
                Ok(None) => {
                    info!("{}: correct", name);
                    self.stats.borrow_mut().inc_corrected_objects();
                },
                Ok(Some(transaction)) => {
                    if self.send_trx {
                        let result = self.send_transaction(&transaction);
                        match result {
                            Ok(signature) => {
                                warn!("{}: updated in trx {}", name, signature);
                                self.signatures.borrow_mut().push(signature);
                                self.stats.borrow_mut().inc_modified_objects();
                                return Ok(Some(signature));
                            },
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
                },
                Err(error) => {
                    error!("{}: wrong object {:?}", name, error);
                    self.stats.borrow_mut().inc_invalid_objects();
                    if self.send_trx {return Err(error);}
                }
            }
        } else {
            match create() {
                Ok(None) => {
                    info!("{}: missed ok", name);
                    self.stats.borrow_mut().inc_corrected_objects();
                },
                Ok(Some(transaction)) => {
                    if self.send_trx {
                        let result = self.send_transaction(&transaction);
                        match result {
                            Ok(signature) => {
                                warn!("{}: created in trx {}", name, signature);
                                self.signatures.borrow_mut().push(signature);
                                self.stats.borrow_mut().inc_created_objects();
                                return Ok(Some(signature));
                            },
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
                },
                Err(error) => {
                    error!("{}: can't be created: {:?}", name, error);
                    self.stats.borrow_mut().inc_invalid_objects();
                    if self.send_trx {return Err(error);}
                }
            }
        }
        Ok(None)
    }
}