//! Airdrop implementation: calls to the Ethereum network.

use std::sync::Arc;

use color_eyre::Report;
use tracing::info;

use ethers::prelude::{
    abigen, Http, LocalWallet, Middleware, Provider, SignerMiddleware, TransactionRequest,
};

use crate::config;

/// Represents packet of information needed for single airdrop operation.
#[derive(Debug, serde::Deserialize)]
pub struct Airdrop {
    wallet: String,
    amount: u64,
}

type Amount = ethers::types::U256;
type Address = ethers::types::Address;

/// Represents a signer account.
type Account = SignerMiddleware<Provider<Http>, LocalWallet>;

/// Generates the type representing ERC20 contract.
impl UniswapV2ERC20<Account> {}
abigen!(UniswapV2ERC20, "abi/UniswapV2ERC20.abi");

/// Processes the aridrop: sends needed transactions into Ethereum.
pub async fn process(airdrop: Airdrop) -> Result<(), Report> {
    info!("Processing {:?}...", airdrop);

    let admin = derive_address(&config::admin_key())?;
    use std::convert::TryFrom as _;
    let provider = Provider::<Http>::try_from(config::ethereum_endpoint())?.with_sender(admin);
    let admin = Arc::new(import_account(provider.clone(), &config::admin_key())?);

    let token_a = address_from_str(&config::token_a())?;
    let token_a = UniswapV2ERC20::new(token_a, admin.clone());
    let token_b = address_from_str(&config::token_b())?;
    let token_b = UniswapV2ERC20::new(token_b, admin.clone());

    let recipient = &airdrop.wallet;
    let amount = Amount::from(airdrop.amount);

    info!("Depositing {} -> token A...", amount);
    let tx = create_transfer_tx(&token_a, recipient, amount).await?;
    info!("Sending transaction for transfer of token A...");
    let tx = provider.send_transaction(tx, None).await?;
    info!("Waiting transaction for transfer of token A...");
    let _receipt = tx.await?;
    //info!("{:?}", receipt);
    info!("OK");

    info!("Depositing {} -> token B...", amount);
    let tx = create_transfer_tx(&token_b, recipient, amount).await?;
    info!("Sending transaction for transfer of token B...");
    let tx = provider.send_transaction(tx, None).await?;
    info!("Waiting transaction for transfer of token B...");
    let _receipt = tx.await?;
    //info!("{:?}", receipt);
    info!("OK");

    Ok(())
}

/// Creates transaction to perform one airdrop operation.
async fn create_transfer_tx(
    token: &UniswapV2ERC20<Account>,
    recipient: &str,
    amount: Amount,
) -> Result<TransactionRequest, Report> {
    let recipient = address_from_str(recipient)?;
    let call = token.transfer(recipient, amount);
    Ok(call.tx)
}

/// Calculates address of given private key.
fn derive_address(priv_key: &str) -> Result<Address, Report> {
    let wallet = priv_key.parse::<LocalWallet>()?;
    Ok(wallet.address())
}

/// Imports account from it's private key.
fn import_account(provider: Provider<Http>, priv_key: &str) -> Result<Account, Report> {
    let wallet = priv_key.parse::<LocalWallet>()?;
    let account = SignerMiddleware::new(provider, wallet);
    Ok(account)
}

/// Converts string representation of address to the H160 hash format.
fn address_from_str(s: &str) -> Result<Address, Report> {
    use std::str::FromStr as _;
    let address = if !s.starts_with("0x") {
        Address::from_str(s)?
    } else {
        Address::from_str(&s[2..])?
    };
    Ok(address)
}
