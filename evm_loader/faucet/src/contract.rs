//! Faucet contract module: interface to ERC20 smart contract.

use ethers::prelude::abigen;

use crate::airdrop::Account;

impl UniswapV2ERC20<Account> {}

abigen!(UniswapV2ERC20, "abi/UniswapV2ERC20.abi");
