use std::collections::BTreeMap;
use borsh::{BorshDeserialize, BorshSerialize};
use mpl_token_metadata::assertions::collection::assert_collection_update_is_valid;
use mpl_token_metadata::assertions::uses::assert_valid_use;
use mpl_token_metadata::utils::{assert_initialized, assert_update_authority_is_correct, assert_data_valid, puff_out_data_fields};
use solana_program::account_info::IntoAccountInfo;
use solana_program::instruction::AccountMeta;
use solana_program::program_option::COption;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;
use spl_token::state::Mint;

use crate::executor::{OwnedAccountInfo};
use solana_program::{
    entrypoint::ProgramResult,
    pubkey::Pubkey, program_error::ProgramError,
    program_pack::Pack
};
use mpl_token_metadata::instruction::{MetadataInstruction, CreateMasterEditionArgs, CreateMetadataAccountArgsV3};
use mpl_token_metadata::state::{Metadata, TokenMetadataAccount, MAX_MASTER_EDITION_LEN, MasterEditionV2, Key, TokenStandard, MAX_METADATA_LEN, CollectionDetails};


pub fn emulate(instruction: &[u8], meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>) -> ProgramResult {

    match MetadataInstruction::try_from_slice(instruction)? {
        MetadataInstruction::CreateMetadataAccountV3(args) => {
            create_metadata_accounts_v3(meta, accounts, &args)
        },
        MetadataInstruction::CreateMasterEditionV3(args) => {
            create_master_edition_v3(meta, accounts, &args)
        },
        _ => Err!(ProgramError::InvalidInstructionData; "Unknown Metaplex instruction")
    }
}


fn create_metadata_accounts_v3(meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>, args: &CreateMetadataAccountArgsV3) -> ProgramResult {
    let metadata_account_key = &meta[0].pubkey;
    let mint_key = &meta[1].pubkey;
    // let _mint_authority_key = &meta[2].pubkey;
    // let _payer_account_key = &meta[3].pubkey;
    let update_authority_key = &meta[4].pubkey;
    // let _system_account_key = &meta[5].pubkey;
    // let _rent_key = &meta[6].pubkey;


    let mut metadata: Metadata = {
        let rent = Rent::get()?;

        let metadata_account = accounts.get_mut(metadata_account_key).unwrap();
        metadata_account.data.resize(MAX_METADATA_LEN, 0);
        metadata_account.owner = mpl_token_metadata::ID;
        metadata_account.lamports = metadata_account.lamports.max(rent.minimum_balance(MAX_METADATA_LEN));

        let metadata_account_info = metadata_account.into_account_info();
        Metadata::from_account_info(&metadata_account_info)?
    };

    let mint: Mint = {
        let mint_info = accounts.get_mut(mint_key).unwrap().into_account_info();
        assert_initialized(&mint_info)?
    };

    let compatible_data = args.data.to_v1();
    assert_data_valid(&compatible_data, update_authority_key, &metadata, false, meta[4].is_signer, false)?;

    metadata.mint = *mint_key;
    metadata.key = Key::MetadataV1;
    metadata.data = compatible_data;
    metadata.is_mutable = args.is_mutable;
    metadata.update_authority = *update_authority_key;

    assert_valid_use(&args.data.uses, &None)?;
    metadata.uses = args.data.uses.clone();

    assert_collection_update_is_valid(false, &None, &args.data.collection)?;
    metadata.collection = args.data.collection.clone();

    if let Some(details) = &args.collection_details {
        match details {
            CollectionDetails::V1 { size: _size } => {
                metadata.collection_details = Some(CollectionDetails::V1 { size: 0 });
            }
        }
    } else {
        metadata.collection_details = None;
    }

    let token_standard = if mint.decimals == 0 { TokenStandard::FungibleAsset } else { TokenStandard::Fungible };
    metadata.token_standard = Some(token_standard);

    puff_out_data_fields(&mut metadata);

    let edition_seeds = &[
        mpl_token_metadata::state::PREFIX.as_bytes(),
        mpl_token_metadata::ID.as_ref(),
        metadata.mint.as_ref(),
        mpl_token_metadata::state::EDITION.as_bytes(),
    ];
    let (_, edition_bump_seed) = Pubkey::find_program_address(edition_seeds, &mpl_token_metadata::ID);
    metadata.edition_nonce = Some(edition_bump_seed);

    {
        let metadata_account = accounts.get_mut(metadata_account_key).unwrap();
        metadata.serialize(&mut metadata_account.data.as_mut_slice())?;
    }

    Ok(())
}


fn create_master_edition_v3(meta: &[AccountMeta], accounts: &mut BTreeMap<Pubkey, OwnedAccountInfo>, args: &CreateMasterEditionArgs) -> ProgramResult {
    let edition_account_key  = &meta[0].pubkey;
    let mint_key             = &meta[1].pubkey;
    let update_authority_key = &meta[2].pubkey;
    // let _mint_authority_key   = &meta[3].pubkey;
    // let _payer_account_key    = &meta[4].pubkey;
    let metadata_account_key = &meta[5].pubkey;
    // let _token_program_key   = &meta[6].pubkey;
    // let _system_account_key  = &meta[7].pubkey;
    // let _rent_key            = &meta[8].pubkey;

    let mut metadata: Metadata = {
        let metadata_info = accounts.get_mut(metadata_account_key).unwrap().into_account_info();
        Metadata::from_account_info(&metadata_info)?
    };

    let mut mint: Mint = {
        let mint_info = accounts.get_mut(mint_key).unwrap().into_account_info();
        assert_initialized(&mint_info)?
    };

    if &metadata.mint != mint_key {
        return Err!(ProgramError::InvalidArgument; "Metaplex: Invalid token mint");
    }

    if mint.decimals != 0 {
        return Err!(ProgramError::InvalidArgument; "Metaplex: mint decimals != 0");
    }
    
    {
        let update_authority_info = accounts.get_mut(update_authority_key).unwrap().into_account_info();
        assert_update_authority_is_correct(&metadata, &update_authority_info)?;
    }

    if mint.supply != 1 {
        return Err!(ProgramError::InvalidArgument; "Metaplex: mint supply != 1");
    }

    
    {
        let rent = Rent::get()?;

        let edition_account = accounts.get_mut(edition_account_key).unwrap();
        edition_account.data.resize(MAX_MASTER_EDITION_LEN, 0);
        edition_account.owner = mpl_token_metadata::ID;
        edition_account.lamports = edition_account.lamports.max(rent.minimum_balance(MAX_MASTER_EDITION_LEN));

        let edition = MasterEditionV2 { key: Key::MasterEditionV2, supply: 0, max_supply: args.max_supply };
        edition.serialize(&mut edition_account.data.as_mut_slice())?;
    }

    {
        let metadata_account = accounts.get_mut(metadata_account_key).unwrap();

        metadata.token_standard = Some(TokenStandard::NonFungible);
        metadata.serialize(&mut metadata_account.data.as_mut_slice())?;
    }

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