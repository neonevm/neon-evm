//! Faucet Solana utilities module.

use std::str::FromStr as _;
use std::sync::{Arc, Mutex};
use std::thread;

use color_eyre::{eyre::eyre, Result};
use tracing::info;

//use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::RpcClient;
use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer as _;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::transaction::Transaction;

use crate::{config, ethereum};

lazy_static::lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::default());
}

/// Creates the signleton instance of RpcClient.
pub fn init_client(url: String) {
    thread::spawn(|| CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new(url)));
}

/// Generates a Solana address by corresponding Ethereum address.
pub fn make_program_address(ether_address: &str) -> Result<Pubkey> {
    let evm_loader_id = Pubkey::from_str(&config::solana_evm_loader())?;
    let (address, _nonce) =
        make_solana_program_address(&ethereum::address_from_str(ether_address)?, &evm_loader_id);
    Ok(address)
}

/// Transfers `amount` of tokens to `recipient` from a known account.
/// Creates the `recipient` account if it doesn't exist.
pub fn transfer_token(
    operator: Keypair,
    token_owner: Pubkey,
    recipient: Pubkey,
    amount: u64,
) -> Result<()> {
    let r = thread::spawn(move || -> Result<()> {
        let client = get_client();
        let mut instructions = vec![];

        let balance = client.get_token_account_balance(&recipient);
        let account_exists = balance.is_ok();
        if account_exists {
            info!("Balance of recipient is {:?}", balance.unwrap());
        } else {
            instructions.push(evm_loader::token::create_associated_token_account(
                &token_owner,
                &token_owner,
                &recipient,
                &evm_loader::token::token_mint::id(),
            ));
        }

        let decimals = 9;
        instructions.push(spl_token::instruction::transfer_checked(
            &spl_token::id(),
            &token_owner,
            &evm_loader::token::token_mint::id(),
            &recipient,
            &token_owner,
            &[],
            amount,
            decimals,
        )?);

        let message = Message::new(&instructions, Some(&operator.pubkey()));
        let mut tx = Transaction::new_unsigned(message);
        let (blockhash, _) = client.get_recent_blockhash()?;
        tx.try_sign(&[&operator], blockhash)?;
        client.send_and_confirm_transaction(&tx)?;

        Ok(())
    })
    .join()
    .expect("thread::spawn join failed");
    if let Err(e) = r {
        return Err(eyre!("{:?}", e));
    }
    Ok(())
}

/// Maps an Ethereum address into a Solana address.
/// Copied here from evm_loader/cli/src/account_storage.rs.
fn make_solana_program_address(
    ether_address: &ethereum::Address,
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &[evm_loader::account_data::ACCOUNT_SEED_VERSION],
            ether_address.as_bytes(),
        ],
        program_id,
    )
}

struct Client(Arc<RpcClient>);

impl Default for Client {
    fn default() -> Client {
        Client(Arc::new(RpcClient::new(String::default())))
    }
}

/// Returns instance of RpcClient.
fn get_client() -> Arc<RpcClient> {
    CLIENT.lock().unwrap().0.clone()
}
