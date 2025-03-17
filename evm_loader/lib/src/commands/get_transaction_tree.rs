use ethnum::U256;
use evm_loader::account::{TransactionTree, TransactionTreeNodeStatus};
use serde::{Deserialize, Serialize};
use solana_sdk::{account_info::AccountInfo, pubkey::Pubkey};
use std::fmt::Display;

use crate::{
    account_storage::account_info,
    rpc::Rpc,
    types::{Address, BalanceAddress},
    NeonResult,
};

use serde_with::{hex::Hex, serde_as, DisplayFromStr};

#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Status {
    #[default]
    Empty,
    Error(String),
    Ok,
}

#[serde_as]
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TreeNode {
    pub status: TransactionTreeNodeStatus,

    #[serde_as(as = "Hex")]
    pub result_hash: [u8; 32],
    #[serde_as(as = "Hex")]
    pub transaction_hash: [u8; 32],

    pub gas_limit: U256,
    pub value: U256,

    pub child_transaction: u16,
    pub success_execute_limit: u16,
    pub parent_count: u16,
}

#[serde_as]
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GetTreeResponse {
    pub status: Status,
    #[serde_as(as = "DisplayFromStr")]
    pub pubkey: Pubkey,

    pub payer: Address,
    pub last_slot: u64,
    pub chain_id: u64,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub balance: U256,
    pub last_index: u16,

    pub transactions: Vec<TreeNode>,
}

impl GetTreeResponse {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            status: Status::Empty,
            ..Self::default()
        }
    }

    pub fn error<T: Display>(error: T) -> Self {
        Self {
            status: Status::Error(error.to_string()),
            ..Self::default()
        }
    }
}

pub fn read_tree(program_id: &Pubkey, info: AccountInfo) -> NeonResult<GetTreeResponse> {
    let tree = TransactionTree::from_account(program_id, info)?;

    let transactions = tree
        .nodes()
        .iter()
        .map(|n| TreeNode {
            status: n.status,
            result_hash: n.result_hash,
            transaction_hash: n.transaction_hash,
            child_transaction: n.child_transaction,
            success_execute_limit: n.success_execute_limit,
            parent_count: n.parent_count,
            gas_limit: n.gas_limit,
            value: n.value,
        })
        .collect();

    Ok(GetTreeResponse {
        status: Status::Ok,
        pubkey: *tree.info().key,
        payer: tree.payer(),
        last_slot: tree.last_slot(),
        chain_id: tree.chain_id(),
        max_fee_per_gas: tree.max_fee_per_gas(),
        max_priority_fee_per_gas: tree.max_priority_fee_per_gas(),
        balance: tree.balance(),
        last_index: tree.last_index(),
        transactions,
    })
}

pub async fn execute(
    rpc: &impl Rpc,
    program_id: &Pubkey,
    origin: BalanceAddress,
    nonce: u64,
) -> NeonResult<GetTreeResponse> {
    let (pubkey, _) =
        TransactionTree::find_address(program_id, origin.address, origin.chain_id, nonce);

    let response = rpc.get_account(&pubkey).await?;
    let Some(mut account) = response else {
        return Ok(GetTreeResponse::empty());
    };

    let info = account_info(&pubkey, &mut account);
    Ok(read_tree(program_id, info).unwrap_or_else(GetTreeResponse::error))
}
