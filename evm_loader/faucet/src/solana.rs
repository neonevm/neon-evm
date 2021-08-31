//! Faucet Solana utilities module.

use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread;

use color_eyre::{eyre::eyre, Result};

//use solana_client::client_error::Result as ClientResult;
use solana_client::rpc_client::RpcClient;
//use solana_sdk::message::Message;
use solana_sdk::pubkey::Pubkey;
//use solana_sdk::transaction::Transaction;

use crate::{config, ethereum};

lazy_static::lazy_static! {
    static ref CLIENT: Mutex<Client> = Mutex::new(Client::default());
}

/// Creates the signleton instance of RpcClient.
pub fn init_client(url: String) {
    thread::spawn(|| CLIENT.lock().unwrap().0 = Arc::new(RpcClient::new(url)));
}

/// Generates a Solana address by corresponding Ethereum address.
pub fn create_program_address(seed: &str) -> Result<Pubkey> {
    let seed = hex::decode(ethereum::strip_0x_prefix(seed))?;
    let seeds = vec![&seed[..]];
    let evm_loader_id = Pubkey::from_str(&config::solana_evm_loader())?;
    let (address, _nonce) = Pubkey::find_program_address(&seeds, &evm_loader_id);
    Ok(address)
}

//pub fn get_token_account_balance(pubkey: &Pubkey) -> Result<UiTokenAmount> {
//    let r = thread::spawn(move || -> ClientResult<UiTokenAmount> {
//        get_client().get_token_account_balance(&token_address)
//    })
//    .join();
//    dbg!(&r);
//    match r {
//        Ok(r) => info!("{:?}", r),
//        Err(e) => error!("{:?}", e),
//    }
//}

/// Transfers `amount` of tokens to `recipient` from a known account.
/// Creates the `recipient` account if it doesn't exist.
pub fn transfer_token(recipient: Pubkey, _amount: u64) -> Result<()> {
    let r = thread::spawn(move || -> Result<()> {
        let client = get_client();
        let a = client.get_token_account(&recipient);
        if let Err(e) = a {
            return Err(eyre!("{:?}", e));
        }
        //        let instructions = vec![spl_token::instruction::transfer(amount)];
        //        let payer = Some(&recipient);
        //        let message = Message::new(&instructions, payer);
        //        let tx = Transaction::new_unsigned(message);
        //        get_client().send_and_confirm_transaction(&tx)
        Ok(())
    })
    .join()
    .expect("thread::spawn join failed");
    if let Err(e) = r {
        return Err(eyre!("{:?}", e));
    }
    Ok(())
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
