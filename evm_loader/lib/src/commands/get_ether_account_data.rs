use evm_loader::{account::EthereumAccount, types::Address};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::fmt::{Display, Formatter};

use crate::{
    account_storage::{account_info, EmulatorAccountStorage},
    errors::NeonError,
    rpc::Rpc,
    NeonResult,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GetEtherAccountDataReturn {
    pub solana_address: String,
    pub address: Address,
    pub bump_seed: u8,
    pub trx_count: u64,
    pub rw_blocked: bool,
    pub balance: String,
    pub generation: u32,
    pub code_size: u32,
    pub code: String,
}

impl Display for GetEtherAccountDataReturn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ address: {}, solana_address: {}, trx_count: {}, balance: {}, generation: {}, code_size: {} }}",
            self.address,
            self.solana_address,
            self.trx_count,
            self.balance,
            self.generation,
            self.code_size,
        )
    }
}

pub async fn execute(
    rpc_client: &dyn Rpc,
    evm_loader: &Pubkey,
    ether_address: &Address,
) -> NeonResult<GetEtherAccountDataReturn> {
    match EmulatorAccountStorage::get_account_from_solana(rpc_client, evm_loader, ether_address)
        .await
    {
        (solana_address, Some(mut acc)) => {
            let acc_info = account_info(&solana_address, &mut acc);
            let account_data = EthereumAccount::from_account(evm_loader, &acc_info).unwrap();
            let contract_code = account_data
                .contract_data()
                .map_or_else(Vec::new, |c| c.code().to_vec());

            Ok(GetEtherAccountDataReturn {
                solana_address: solana_address.to_string(),
                address: account_data.address,
                bump_seed: account_data.bump_seed,
                trx_count: account_data.trx_count,
                rw_blocked: account_data.rw_blocked,
                balance: account_data.balance.to_string(),
                generation: account_data.generation,
                code_size: account_data.code_size,
                code: hex::encode(contract_code),
            })
        }
        (solana_address, None) => Err(NeonError::AccountNotFound(solana_address)),
    }
}
