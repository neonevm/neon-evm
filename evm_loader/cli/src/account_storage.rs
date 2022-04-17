use std::{
    cell::RefCell,
    cell::RefMut,
    collections::HashMap,
    rc::Rc,
    convert::TryInto,
};

use log::{error, info, trace, warn};

use evm::{H160, U256, H256, Transfer};
use evm::backend::Apply;
use serde::{Deserialize, Serialize};


use solana_program::{
    instruction::AccountMeta,
    sysvar::recent_blockhashes,
};

use solana_sdk::{
    account::Account,
    account_info::AccountInfo,
    pubkey::Pubkey,
    sysvar::rent
};

use evm_loader::{
    account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumContract, ERC20Allowance, token},
    account_storage::{AccountStorage},
    executor_state::{ERC20Approve, SplApprove, SplTransfer},
    precompile_contracts::is_precompile_address,
    hamt::Hamt,
};
use evm_loader::executor_state::Withdraw;

use crate::Config;
use crate::NeonCliResult;
use crate::NeonCliError;

use spl_associated_token_account::{get_associated_token_address};

#[derive(Debug, Clone)]
pub struct TokenAccount {
    owner: Pubkey,
    contract: Pubkey,
    mint: Pubkey,
    key: Pubkey,
    new: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenAccountJSON {
    owner: String,
    contract: String,
    mint: String,
    key: String,
    new: bool
}
impl From<TokenAccount> for TokenAccountJSON {
    fn from(account: TokenAccount) -> Self {
        Self {
            owner: bs58::encode(&account.owner).into_string(),
            contract: bs58::encode(&account.contract).into_string(),
            mint: bs58::encode(&account.mint).into_string(),
            key: bs58::encode(&account.key).into_string(),
            new: account.new,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountJSON {
    address: String,
    account: String,
    contract: Option<String>,
    writable: bool,
    new: bool,
    code_size: Option<usize>,
    code_size_current: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SolanaAccountJSON {
    /// An account's public key
    pub pubkey: String,
    /// True if an Instruction requires a Transaction signature matching `pubkey`.
    pub is_signer: bool,
    /// True if the `pubkey` can be loaded as a read-write account.
    pub is_writable: bool,
}
impl From<AccountMeta> for SolanaAccountJSON {
    fn from(account_meta: AccountMeta) -> Self {
        Self {
            pubkey: bs58::encode(&account_meta.pubkey).into_string(),
            is_signer: account_meta.is_signer,
            is_writable: account_meta.is_writable,
        }
    }
}

struct SolanaAccount {
    account: Account,
    code_account: Option<Account>,
    key: Pubkey,
    writable: bool,
    code_size: Option<usize>,
    code_size_current: Option<usize>,
}

struct SolanaNewAccount {
    key: Pubkey,
    writable: bool,
    code_size: Option<usize>,
}

impl SolanaAccount {
    pub fn new(account: Account, key: Pubkey, code_account: Option<Account>) -> Self {
        trace!("SolanaAccount::new");
        Self{account, key, writable: false, code_account, code_size: None, code_size_current : None}
    }
}

impl SolanaNewAccount {
    pub const fn new(key: Pubkey) -> Self {
        Self{key, writable: false, code_size: None}
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct EmulatorAccountStorage<'a> {
    accounts: RefCell<HashMap<H160, SolanaAccount>>,
    new_accounts: RefCell<HashMap<H160, SolanaNewAccount>>,
    pub solana_accounts: RefCell<HashMap<Pubkey, AccountMeta>>,
    pub token_accounts: RefCell<HashMap<Pubkey, TokenAccount>>,
    config: &'a Config,
    block_number: u64,
    block_timestamp: i64,
    token_mint: Pubkey,
    chain_id: u64,
}

impl<'a> EmulatorAccountStorage<'a> {
    pub fn new(config: &'a Config, token_mint: Pubkey, chain_id: u64) -> EmulatorAccountStorage {
        trace!("backend::new");

        let slot = if let Ok(slot) = config.rpc_client.get_slot() {
            trace!("Got slot");
            trace!("Slot {}", slot);
            slot
        }
        else {
            error!("Get slot error");
            0
        };

        let timestamp = if let Ok(timestamp) = config.rpc_client.get_block_time(slot) {
            trace!("Got timestamp");
            trace!("timestamp {}", timestamp);
            timestamp
        } else {
            error!("Get timestamp error");
            0
        };

        Self {
            accounts: RefCell::new(HashMap::new()),
            new_accounts: RefCell::new(HashMap::new()),
            solana_accounts: RefCell::new(HashMap::new()),
            token_accounts: RefCell::new(HashMap::new()),
            config,
            block_number: slot,
            block_timestamp: timestamp,
            token_mint,
            chain_id,
        }
    }

    pub fn get_account_from_solana(config: &'a Config, address: &H160) -> Option<(Account, Option<Account>)> {
        let (solana_address, _solana_nonce) = make_solana_program_address(address, &config.evm_loader);
        info!("Not found account for 0x{} => {}", &hex::encode(&address.as_fixed_bytes()), &solana_address.to_string());

        if let Ok(mut acc) = config.rpc_client.get_account(&solana_address) {
            trace!("Account found");
            trace!("Account data len {}", acc.data.len());
            trace!("Account owner {}", acc.owner);

            let code_address = {
                let info = account_info(&solana_address, &mut acc);
                let account_data = EthereumAccount::from_account(&config.evm_loader, &info).ok()?;
                account_data.code_account
            };

            let code_account = if let Some(code_address) = code_address {
                info!("code_account != None");
                trace!("account key:  {:?}", &solana_address);
                trace!("code account: {:?}", &code_address);

                config.rpc_client.get_account(&code_address).ok()
            } else {
                info!("code_account == None");
                None
            };

            Some((acc, code_account))
        }
        else {
            warn!("Account not found {}", &address.to_string());

            None
        }
    }

    fn create_acc_if_not_exists(&self, address: &H160, writable: bool) -> bool {
        let mut accounts = self.accounts.borrow_mut();
        let mut new_accounts = self.new_accounts.borrow_mut();

        if let Some(ref mut account) = accounts.get_mut(address) {
            account.writable |= writable;
            true
        } else {
            let (solana_address, _solana_nonce) = make_solana_program_address(address, &self.config.evm_loader);
            if let Some((acc, code_account)) = Self::get_account_from_solana(self.config, address) {
                let mut account = SolanaAccount::new(acc, solana_address, code_account);
                account.writable |= writable;

                accounts.insert(*address, account);
                true
            }
            else {
                if let Some(ref mut account) = new_accounts.get_mut(address) {
                    account.writable |= writable;
                } else {
                    warn!("Account not found {}", &address.to_string());
                    let mut account = SolanaNewAccount::new(solana_address);
                    account.writable = writable;

                    new_accounts.insert(*address, account);
                }

                false
            }
        }
    }


    #[allow(clippy::too_many_lines)]
    pub fn apply<A, I>(&self, values: A) -> NeonCliResult
            where
                A: IntoIterator<Item=Apply<I>>,
                I: IntoIterator<Item=(U256, U256)>,
    {
        for apply in values {
            match apply {
                Apply::Modify {address, nonce, code_and_valids, storage, reset_storage} => {
                    let mut storage_iter = storage.into_iter().peekable();
                    let exist_items: bool = matches!(storage_iter.peek(), Some(_));

                    let hamt_size = |code_data : &[u8], hamt_begin : usize| ->usize {
                        let buffer = RefCell::new(vec![0_u8; 10_485_760]);
                        let mut buffer_ref = buffer.borrow_mut();
                        buffer_ref[0..code_data.len()].copy_from_slice(code_data);

                        let (_, hamt_data) = RefMut::map_split(buffer_ref, |b| b.split_at_mut(hamt_begin));

                        let mut storage = Hamt::new(hamt_data).unwrap();
                        for (key, value) in storage_iter {
                            info!("Storage value: {} = {}", &key.to_string(), &value.to_string());
                            storage.insert(key, value).unwrap();
                        }

                        storage.last_used() as usize
                    };

                    if nonce > U256::from(u64::MAX) {
                        return Err(NeonCliError::TrxCountOverflow);
                    }
                        
                    let mut accounts = self.accounts.borrow_mut();
                    let mut new_accounts = self.new_accounts.borrow_mut();
                    if let Some(ref mut acc) = accounts.get_mut(&address) {
                        let acc_info = account_info(&acc.key, &mut acc.account);
                        let acc_desc = EthereumAccount::from_account(&self.config.evm_loader, &acc_info)?;


                        if let Some(ref mut code_account) = acc.code_account{
                            let code_account_data = code_account.data.clone();

                            let code_key = acc_desc.code_account.unwrap();
                            let code_info = account_info(&code_key, code_account);
                            let contract = EthereumContract::from_account(&self.config.evm_loader, &code_info)?;

                            let (code_begin, code_size, valids_size) = if let Some((code, valids)) = &code_and_valids {
                                if contract.code_size != 0 {
                                    return Err(NeonCliError::AccountAlreadyInitialized(acc.key, code_key));
                                }
                                
                                (EthereumContract::SIZE, code.len(), valids.len())
                            }
                            else{
                                let code_size = contract.code_size as usize;
                                let valids_size = (code_size / 8) + 1;
                                (EthereumContract::SIZE, code_size, valids_size)
                            };

                            let hamt_begin = code_begin + code_size + valids_size;

                            acc.code_size_current = Some(code_account_data.len());
                            acc.code_size = Some(hamt_begin + hamt_size(&code_account_data, hamt_begin));

                            let trx_count: u64 = nonce.as_u64();
                            if reset_storage || exist_items || code_and_valids.is_some() || acc_desc.trx_count != trx_count {
                                acc.writable = true;
                            }
                        }
                        else if let Some((code, valids)) = &code_and_valids {
                            if acc_desc.trx_count != 0 {
                                return Err(NeonCliError::DeploymentToExistingAccount(address));
                            }

                            let hamt_begin = EthereumContract::SIZE + code.len() + valids.len();
                            acc.code_size = Some(hamt_begin + hamt_size(&[0_u8; 0], hamt_begin));
                            acc.code_size_current = Some(0);
                            acc.writable = true;
                        }
                        else {
                            if reset_storage || exist_items {
                                return Err(NeonCliError::ContractAccountExpected(address));
                            }

                            acc.writable = true;
                        }
                    }
                    else if let Some(acc) = new_accounts.get_mut(&address) {
                        if let Some((code, valids)) = &code_and_valids {
                            let hamt_begin = EthereumContract::SIZE + code.len() + valids.len();
                            acc.code_size = Some(hamt_begin + hamt_size(&[0_u8; 0], hamt_begin));
                        }
                        else if reset_storage || exist_items {
                            return Err(NeonCliError::ContractAccountExpected(address));
                        }

                        acc.writable = true;
                    }
                    else {
                        warn!("Account not found {}", &address.to_string());
                    }
                    info!("Modify: {} {} {}", &address.to_string(), &nonce.as_u64(), &reset_storage.to_string());
                },
                Apply::Delete {address} => {
                    info!("Delete: {}", address);

                    self.create_acc_if_not_exists(&address, true);
                },
            }
        };
        Ok(())
    }

    pub fn apply_transfers(&self, transfers: Vec<Transfer>) {
        for transfer in transfers {
            self.create_acc_if_not_exists(&transfer.source, true);
            self.create_acc_if_not_exists(&transfer.target, true);
        }
    }

    pub fn apply_spl_transfers(&self, transfers: Vec<SplTransfer>) {
        let mut token_accounts = self.token_accounts.borrow_mut();

        for transfer in transfers {
            self.create_acc_if_not_exists(&transfer.source, false);
            self.create_acc_if_not_exists(&transfer.target, true);

            let mut new_accounts = self.new_accounts.borrow_mut();
            if let Some(ref mut account) = new_accounts.get_mut(&transfer.target) {
                account.writable = true;
            }

            let (contract_solana_address, _) = make_solana_program_address(&transfer.contract, &self.config.evm_loader);

            let (source_solana_address, _) = make_solana_program_address(&transfer.source, &self.config.evm_loader);
            token_accounts.entry(transfer.source_token).or_insert(
                TokenAccount {
                    owner: source_solana_address,
                    contract: contract_solana_address,
                    mint: transfer.mint,
                    key: transfer.source_token,
                    new: false
                }
            );

            let ui_token_account = self.config.rpc_client.get_token_account(&transfer.target_token);
            let target_token_exists = ui_token_account.map(|r| r.is_some()).unwrap_or(false);

            let (target_solana_address, _) = make_solana_program_address(&transfer.target, &self.config.evm_loader);
            token_accounts.entry(transfer.target_token).or_insert(
                TokenAccount {
                    owner: target_solana_address,
                    contract: contract_solana_address,
                    mint: transfer.mint,
                    key: transfer.target_token,
                    new: !target_token_exists
                }
            );
        }
    }

    pub fn apply_spl_approves(&self, approves: Vec<SplApprove>) {
        let mut token_accounts = self.token_accounts.borrow_mut();

        let mut solana_accounts = self.solana_accounts.borrow_mut();
        for approve in approves {
            self.create_acc_if_not_exists(&approve.owner, false);
            solana_accounts.entry(approve.spender).or_insert_with(|| AccountMeta::new_readonly(approve.spender, false));

            let (contract_solana_address, _) = make_solana_program_address(&approve.contract, &self.config.evm_loader);
            let (owner_solana_address, _) = make_solana_program_address(&approve.owner, &self.config.evm_loader);

            let (token_address, _) = self.get_erc20_token_address(&approve.owner, &approve.contract, &approve.mint);
            let ui_token_account = self.config.rpc_client.get_token_account(&token_address);
            let token_exists = ui_token_account.map(|r| r.is_some()).unwrap_or(false);

            token_accounts.entry(token_address).or_insert(
                TokenAccount {
                    owner: owner_solana_address,
                    contract: contract_solana_address,
                    mint: approve.mint,
                    key: token_address,
                    new: !token_exists
                }
            );
        }
    }

    pub fn apply_erc20_approves(&self, approves: Vec<ERC20Approve>) {
        let mut solana_accounts = self.solana_accounts.borrow_mut();

        for approve in approves {
            let (address, _) = self.get_erc20_allowance_address(
                &approve.owner,
                &approve.spender,
                &approve.contract,
                &approve.mint
            );

            solana_accounts.insert(address, AccountMeta::new(address, false));
        }
    }

    pub fn apply_withdrawals(&self, withdrawals: Vec<Withdraw>, token_mint: &Pubkey) {
        if withdrawals.is_empty() {
            return;
        }

        let mut solana_accounts = self.solana_accounts.borrow_mut();

        solana_accounts.entry(*token_mint).or_insert_with(|| AccountMeta::new_readonly(*token_mint, false));

        let (authority, _) = Pubkey::find_program_address(&[b"Deposit"], &self.config.evm_loader);
        solana_accounts.entry(authority).or_insert_with(|| AccountMeta::new_readonly(authority, false));

        let pool_address = get_associated_token_address(
            &authority,
            token_mint
        );
        solana_accounts.insert(pool_address, AccountMeta::new(pool_address, false));

        solana_accounts.entry(rent::id()).or_insert_with(|| AccountMeta::new_readonly(rent::id(), false));

        let assoc_token_prog_id = spl_associated_token_account::id();
        solana_accounts.entry(assoc_token_prog_id).or_insert_with(|| AccountMeta::new_readonly(assoc_token_prog_id, false));

        for withdraw in withdrawals {
            solana_accounts.entry(withdraw.dest).or_insert_with(|| AccountMeta::new_readonly(withdraw.dest, false));
            solana_accounts.insert(withdraw.dest_neon, AccountMeta::new(withdraw.dest_neon, false));
        }
    }

    pub fn get_used_accounts(&self) -> Vec<AccountJSON>
    {
        let mut arr = Vec::new();

        let mut accounts = self.accounts.borrow_mut();
        for (address, acc) in accounts.iter_mut() {
            let (solana_address, _solana_nonce) = make_solana_program_address(address, &self.config.evm_loader);

            let account_info = account_info(&acc.key, &mut acc.account);
            let account_data = EthereumAccount::from_account(&self.config.evm_loader, &account_info).unwrap();

            let contract_address = account_data.code_account;

            if !is_precompile_address(address) {
                arr.push(AccountJSON{
                        address: "0x".to_string() + &hex::encode(&address.to_fixed_bytes()),
                        writable: acc.writable,
                        new: false,
                        account: solana_address.to_string(),
                        contract: contract_address.map(|v| v.to_string()),
                        code_size: acc.code_size,
                        code_size_current: acc.code_size_current,
                });
            }
        }

        let new_accounts = self.new_accounts.borrow();
        for (address, acc) in new_accounts.iter() {
            if !is_precompile_address(address) {
                arr.push(AccountJSON{
                        address: "0x".to_string() + &hex::encode(&address.to_fixed_bytes()),
                        writable: acc.writable,
                        new: true,
                        account: acc.key.to_string(),
                        contract: None,
                        code_size: acc.code_size,
                        code_size_current : None,
                });
            }
        }

        arr
    }

    fn ethereum_account_map_or<F, D>(&self, address: &H160, default: D, f: F) -> D 
    where 
        F: FnOnce(&EthereumAccount) -> D
    {
        self.create_acc_if_not_exists(address, false);

        let mut accounts = self.accounts.borrow_mut();

        if let Some(ref mut account) = accounts.get_mut(address) {
            let info = account_info(&account.key, &mut account.account);
            let ethereum_account = EthereumAccount::from_account(&self.config.evm_loader, &info).unwrap();
            f(&ethereum_account)
        } else {
            default
        }
    }

    fn ethereum_contract_map_or<F, D>(&self, address: &H160, default: D, f: F) -> D 
    where 
        F: FnOnce(&EthereumContract) -> D
    {
        self.create_acc_if_not_exists(address, false);

        let mut accounts = self.accounts.borrow_mut();

        if let Some(ref mut account) = accounts.get_mut(address) {
            let info = account_info(&account.key, &mut account.account);
            let ethereum_account = EthereumAccount::from_account(&self.config.evm_loader, &info).unwrap();

            if let Some(ref mut code_account) = account.code_account {
                let code_key = ethereum_account.code_account.unwrap();
                let code_info = account_info(&code_key, code_account);
                let ethereum_contract = EthereumContract::from_account(&self.config.evm_loader, &code_info).unwrap();

                f(&ethereum_contract)
            } else {
                default
            }
        } else {
            default
        }
    }
}

pub fn make_solana_program_address(
    ether_address: &H160,
    program_id: &Pubkey
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[&[ACCOUNT_SEED_VERSION], ether_address.as_bytes()], program_id)
}


impl<'a> AccountStorage for EmulatorAccountStorage<'a> {
    fn token_mint(&self) -> &Pubkey { &self.token_mint }

    fn program_id(&self) -> &Pubkey {
        &self.config.evm_loader
    }

    fn block_number(&self) -> U256 {
        self.block_number.into()
    }

    fn block_timestamp(&self) -> U256 {
        self.block_timestamp.into()
    }

    fn block_hash(&self, number: U256) -> H256 { 
        info!("Get block hash {}", number);
        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.insert(recent_blockhashes::ID, AccountMeta::new(recent_blockhashes::ID, false));

        if self.block_number <= number.as_u64() {
            return H256::default();
        }

        if let Ok(timestamp) = self.config.rpc_client.get_block(number.as_u64()) {
            H256::from_slice(&bs58::decode(timestamp.blockhash).into_vec().unwrap())
        } else {
            warn!("Got error trying to get block hash");
            H256::default()
        }
    }

    fn exists(&self, address: &H160) -> bool {
        self.create_acc_if_not_exists(address, false);

        let accounts = self.accounts.borrow();
        accounts.contains_key(address)
    }

    fn nonce(&self, address: &H160) -> U256 {
        self.ethereum_account_map_or(address, 0_u64, |a| a.trx_count).into()
    }

    fn balance(&self, address: &H160) -> U256 {
        self.ethereum_account_map_or(address, U256::zero(), |a| a.balance)
    }

    fn code_size(&self, address: &H160) -> usize {
        self.ethereum_contract_map_or(address, 0_u32, |c| c.code_size)
            .try_into()
            .expect("usize is 8 bytes")
    }

    fn code_hash(&self, address: &H160) -> H256 {
        self.ethereum_contract_map_or(address, 
            H256::default(), 
            |c| evm_loader::utils::keccak256_h256(&c.extension.code)
        )
    }

    fn code(&self, address: &H160) -> Vec<u8> {
        self.ethereum_contract_map_or(address,
            Vec::new(),
            |c| c.extension.code.to_vec()
        )
    }

    fn valids(&self, address: &H160) -> Vec<u8> {
        self.ethereum_contract_map_or(address,
            Vec::new(),
            |c| c.extension.valids.to_vec()
        )
    }

    fn storage(&self, address: &H160, index: &U256) -> U256 {
        self.ethereum_contract_map_or(address,
            None,
            |c| c.extension.storage.find(*index)
        ).unwrap_or_else(U256::zero)
    }

    fn get_spl_token_balance(&self, token_account: &Pubkey) -> u64 {
        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.entry(*token_account).or_insert_with(|| AccountMeta::new_readonly(*token_account, false));

        self.config.rpc_client.get_token_account_balance(token_account)
            .map(|token| token.amount.parse::<u64>().unwrap() )
            .unwrap_or(0_u64)
    }

    fn get_spl_token_supply(&self, token_mint: &Pubkey) -> u64 {
        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.entry(*token_mint).or_insert_with(|| AccountMeta::new_readonly(*token_mint, false));

        self.config.rpc_client.get_token_supply(token_mint)
            .map(|token| token.amount.parse::<u64>().unwrap() )
            .unwrap_or(0_u64)
    }

    fn get_spl_token_decimals(&self, token_mint: &Pubkey) -> u8 {
        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.entry(*token_mint).or_insert_with(|| AccountMeta::new_readonly(*token_mint, false));

        if let Ok(ref mut account) = self.config.rpc_client.get_account(token_mint) {
            let info = account_info(token_mint, account);
            token::Mint::from_account(&info).map_or(0_u8, |mint| mint.decimals)
        } else {
            0_u8
        }
    }

    fn get_erc20_allowance(&self, owner: &H160, spender: &H160, contract: &H160, mint: &Pubkey) -> U256 {
        let (address, _) = self.get_erc20_allowance_address(owner, spender, contract, mint);

        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.entry(address).or_insert_with(|| AccountMeta::new_readonly(address, false));

        if let Ok(ref mut account) = self.config.rpc_client.get_account(&address) {
            let info = account_info(&address, account);
            ERC20Allowance::from_account(&self.config.evm_loader, &info).map_or_else(|_| U256::zero(), |a| a.value)
        } else {
            U256::zero()
        }        
    }

    fn query_account(&self, address: &Pubkey, data_offset: usize, data_len: usize) -> Option<evm_loader::query::Value> {
        let mut solana_accounts = self.solana_accounts.borrow_mut();
        solana_accounts.entry(*address).or_insert_with(|| AccountMeta::new_readonly(*address, false));

        if let Ok(account) = self.config.rpc_client.get_account(address) {
            if account.owner == self.config.evm_loader { // NeonEVM accounts may be already borrowed
                return None;
            }

            Some(evm_loader::query::Value {
                owner: account.owner,
                length: account.data.len(),
                lamports: account.lamports,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                offset: data_offset,
                data: evm_loader::query::clone_chunk(&account.data, data_offset, data_len),
            })
        } else {
            None
        }
    }

    fn solana_accounts_space(&self, address: &H160) -> (usize, usize) {
        let account_space = {
            self.ethereum_account_map_or(address, 0, |a| a.info.data_len())
        };

        let contract_space = {
            self.ethereum_contract_map_or(address,
                0,
                |a| {
                    EthereumContract::SIZE
                        + a.extension.code.len()
                        + a.extension.valids.len()
                        + a.extension.storage.buffer_len()
            })
        };

        (account_space, contract_space)
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
}


/// Creates new instance of `AccountInfo` from `Account`.
pub fn account_info<'a>(key: &'a Pubkey, account: &'a mut Account) -> AccountInfo<'a> {
    AccountInfo {
        key,
        is_signer: false,
        is_writable: false,
        lamports: Rc::new(RefCell::new(&mut account.lamports)),
        data: Rc::new(RefCell::new(&mut account.data)),
        owner: &account.owner,
        executable: account.executable,
        rent_epoch: account.rent_epoch,
    }
}
