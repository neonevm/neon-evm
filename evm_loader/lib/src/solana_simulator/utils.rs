use super::error::Error;
use log::debug;
use solana_runtime::{
    bank::Bank,
    genesis_utils::{create_genesis_config_with_leader_ex, GenesisConfigInfo},
};
use solana_sdk::{
    account::Account,
    account_utils::StateMut,
    address_lookup_table::{
        self,
        state::{AddressLookupTable, LookupTableMeta},
    },
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    fee_calculator::DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE,
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    sysvar,
};

use crate::rpc::Rpc;

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum SyncState {
    No,
    Yes,
}

pub async fn genesis_config_info(
    rpc: &impl Rpc,
    sync: SyncState,
    mint_sol: f64,
) -> Result<GenesisConfigInfo, Error> {
    let rent = sysvar::rent::Rent::default();
    let fee_rate_governor = solana_sdk::fee_calculator::FeeRateGovernor {
        // Initialize with a non-zero fee
        lamports_per_signature: DEFAULT_TARGET_LAMPORTS_PER_SIGNATURE / 2,
        ..solana_sdk::fee_calculator::FeeRateGovernor::default()
    };
    let validator_pubkey = Pubkey::new_unique();
    let validator_stake_lamports = rent
        .minimum_balance(solana_sdk::vote::state::VoteState::size_of())
        + sol_to_lamports(1_000_000.0);

    let mint_keypair = Keypair::new();
    let voting_keypair = Keypair::new();

    let mut genesis_config = create_genesis_config_with_leader_ex(
        sol_to_lamports(mint_sol),
        &mint_keypair.pubkey(),
        &validator_pubkey,
        &voting_keypair.pubkey(),
        &Pubkey::new_unique(),
        validator_stake_lamports,
        42,
        fee_rate_governor,
        rent,
        solana_sdk::genesis_config::ClusterType::Development,
        vec![],
    );

    if sync == SyncState::Yes {
        for feature in rpc.get_deactivated_solana_features().await? {
            genesis_config.accounts.remove(&feature);
        }
    }

    Ok(GenesisConfigInfo {
        genesis_config,
        mint_keypair,
        voting_keypair,
        validator_pubkey,
    })
}

pub async fn sync_sysvar_accounts(rpc: &impl Rpc, bank: &Bank) -> Result<(), Error> {
    let keys = sysvar::ALL_IDS.clone();
    let accounts = rpc.get_multiple_accounts(&keys).await?;
    for (key, account) in keys.into_iter().zip(accounts) {
        let Some(account) = account else {
            continue;
        };

        match key {
            sysvar::clock::ID => {
                use sysvar::clock::Clock;

                let clock: Clock = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&clock);
            }
            sysvar::epoch_schedule::ID => {
                use sysvar::epoch_schedule::EpochSchedule;

                let epoch_schedule: EpochSchedule = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&epoch_schedule);
            }
            sysvar::rent::ID => {
                use sysvar::rent::Rent;

                let rent: Rent = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&rent);
            }
            sysvar::rewards::ID => {
                use sysvar::rewards::Rewards;

                let rewards: Rewards = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&rewards);
            }
            sysvar::slot_hashes::ID => {
                use sysvar::slot_hashes::SlotHashes;

                let slot_hashes: SlotHashes = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&slot_hashes);
            }
            sysvar::slot_history::ID => {
                use sysvar::slot_history::SlotHistory;

                let slot_history: SlotHistory = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&slot_history);
            }
            sysvar::stake_history::ID => {
                use sysvar::stake_history::StakeHistory;

                let stake_history: StakeHistory = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&stake_history);
            }
            #[allow(deprecated)]
            id if sysvar::fees::check_id(&id) => {
                use sysvar::fees::Fees;

                let fees: Fees = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&fees);
            }
            #[allow(deprecated)]
            id if sysvar::recent_blockhashes::check_id(&id) => {
                use sysvar::recent_blockhashes::RecentBlockhashes;

                let recent_blockhashes: RecentBlockhashes = bincode::deserialize(&account.data)?;
                bank.set_sysvar_for_tests(&recent_blockhashes);
            }
            _ => {}
        }
    }

    Ok(())
}

pub fn program_data_address(account: &Account) -> Result<Pubkey, Error> {
    assert!(account.executable);
    assert!(account.owner == bpf_loader_upgradeable::id());

    let UpgradeableLoaderState::Program {
        programdata_address,
        ..
    } = account.state()?
    else {
        return Err(Error::ProgramAccountError);
    };

    Ok(programdata_address)
}

pub fn reset_program_data_slot(account: &mut Account) -> Result<(), Error> {
    assert!(account.owner == bpf_loader_upgradeable::id());

    let UpgradeableLoaderState::ProgramData {
        slot,
        upgrade_authority_address,
    } = account.state()?
    else {
        return Err(Error::ProgramAccountError);
    };

    debug!(
        "slot_before_update: slot={slot} upgrade_authority_address={upgrade_authority_address:?}"
    );

    let new_state = UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address,
    };
    account.set_state(&new_state)?;

    debug!(
        "slot_after_update: slot={slot} upgrade_authority_address={upgrade_authority_address:?}"
    );

    Ok(())
}

pub fn reset_alt_slot(account: &mut Account) -> Result<(), Error> {
    assert!(account.owner == address_lookup_table::program::id());

    let lookup_table = AddressLookupTable::deserialize(&account.data)?;
    let metadata = LookupTableMeta {
        last_extended_slot: 0,
        ..lookup_table.meta
    };

    AddressLookupTable::overwrite_meta_data(&mut account.data, metadata)?;

    Ok(())
}
