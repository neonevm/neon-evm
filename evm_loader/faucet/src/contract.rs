//! faucet contract module: interface to the ERC20 smart contract.

use crate::server::Account;
use ethers::prelude::*;

impl UniswapV2ERC20<Account> {}

abigen!(UniswapV2ERC20, "abi/UniswapV2ERC20.abi");
