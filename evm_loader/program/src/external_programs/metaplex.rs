use crate::error::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use mpl_token_metadata::{
    accounts::{MasterEdition, Metadata},
    instructions::{CreateMasterEditionV3InstructionArgs, CreateMetadataAccountV3InstructionArgs},
    programs::MPL_TOKEN_METADATA_ID,
    types::{Key, TokenStandard},
};
use solana_program::instruction::AccountMeta;
use solana_program::program_option::COption;
use solana_program::rent::Rent;
use solana_program::{account_info::IntoAccountInfo, program_pack::Pack};
use std::collections::BTreeMap;

use crate::executor::OwnedAccountInfo;
use solana_program::pubkey::Pubkey;

pub fn emulate(
    instruction: &[u8],
    meta: &[AccountMeta],
    accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>,
    rent: &Rent,
) -> Result<()> {
    let discriminator = instruction[0];
    let data = &instruction[1..];

    match discriminator {
        33 => {
            let args = CreateMetadataAccountV3InstructionArgs::try_from_slice(data)?;
            create_metadata_accounts_v3(meta, accounts, args, rent)
        }
        17 => {
            let args = CreateMasterEditionV3InstructionArgs::try_from_slice(data)?;
            create_master_edition_v3(meta, accounts, args.max_supply, rent)
        }
        _ => Err("Unknown Metaplex instruction".into()),
    }
}

fn create_metadata_accounts_v3(
    meta: &[AccountMeta],
    accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>,
    args: CreateMetadataAccountV3InstructionArgs,
    rent: &Rent,
) -> Result<()> {
    let metadata_account_key = &meta[0].pubkey;
    let mint_key = &meta[1].pubkey;
    // let _mint_authority_key = &meta[2].pubkey;
    // let _payer_account_key = &meta[3].pubkey;
    let update_authority_key = &meta[4].pubkey;
    // let _system_account_key = &meta[5].pubkey;
    // let _rent_key = &meta[6].pubkey;

    let mint = {
        let mint_info = accounts.get_mut(mint_key).unwrap().into_account_info();
        crate::account::token::Mint::from_account(&mint_info)?.into_data()
    };

    let (_, edition_bump_seed) = MasterEdition::find_pda(mint_key);

    let metadata = Metadata {
        key: Key::MetadataV1,
        update_authority: *update_authority_key,
        mint: *mint_key,
        name: args.data.name,
        symbol: args.data.symbol,
        uri: args.data.uri,
        seller_fee_basis_points: args.data.seller_fee_basis_points,
        creators: args.data.creators,
        primary_sale_happened: false,
        is_mutable: args.is_mutable,
        edition_nonce: Some(edition_bump_seed),
        token_standard: if mint.decimals == 0 {
            Some(TokenStandard::FungibleAsset)
        } else {
            Some(TokenStandard::Fungible)
        },
        collection: args.data.collection,
        uses: args.data.uses,
        collection_details: args.collection_details,
        programmable_config: None,
    };

    let metadata_account = accounts.get_mut(metadata_account_key).unwrap();
    metadata_account.owner = MPL_TOKEN_METADATA_ID;
    metadata_account.data = metadata.try_to_vec()?;
    metadata_account.lamports = rent.minimum_balance(metadata_account.data.len());

    Ok(())
}

fn create_master_edition_v3(
    meta: &[AccountMeta],
    accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>,
    max_supply: Option<u64>,
    rent: &Rent,
) -> Result<()> {
    let edition_account_key = &meta[0].pubkey;
    let mint_key = &meta[1].pubkey;
    // let update_authority_key = &meta[2].pubkey;
    // let _mint_authority_key   = &meta[3].pubkey;
    // let _payer_account_key    = &meta[4].pubkey;
    let metadata_account_key = &meta[5].pubkey;
    // let _token_program_key   = &meta[6].pubkey;
    // let _system_account_key  = &meta[7].pubkey;
    // let _rent_key            = &meta[8].pubkey;

    let mut metadata: Metadata = {
        let metadata_account = accounts.get_mut(metadata_account_key).unwrap();
        Metadata::from_bytes(&metadata_account.data)?
    };

    let mut mint = {
        let mint_info = accounts.get_mut(mint_key).unwrap().into_account_info();
        crate::account::token::Mint::from_account(&mint_info)?.into_data()
    };

    if &metadata.mint != mint_key {
        return Err("Metaplex: Invalid token mint".into());
    }

    if mint.decimals != 0 {
        return Err("Metaplex: mint decimals != 0".into());
    }

    if mint.supply != 1 {
        return Err("Metaplex: mint supply != 1".into());
    }

    let edition = MasterEdition {
        key: Key::MasterEditionV2,
        supply: 0,
        max_supply,
    };

    // Master Edition Account
    {
        let edition_account = accounts.get_mut(edition_account_key).unwrap();
        edition_account.owner = MPL_TOKEN_METADATA_ID;
        edition_account.data = edition.try_to_vec()?;
        edition_account.lamports = rent.minimum_balance(edition_account.data.len());
    }
    // Metadata Account
    {
        let metadata_account = accounts.get_mut(metadata_account_key).unwrap();

        metadata.token_standard = Some(TokenStandard::NonFungible);
        metadata.serialize(&mut metadata_account.data.as_mut_slice())?;
    }
    // Mint Account
    {
        mint.mint_authority = COption::Some(*edition_account_key);
        if mint.freeze_authority.is_some() {
            mint.freeze_authority = COption::Some(*edition_account_key);
        }

        let mint_account = accounts.get_mut(mint_key).unwrap();
        mint.pack_into_slice(&mut mint_account.data);
    }

    Ok(())
}
