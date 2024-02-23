use evm_loader::account::{
    legacy::{
        ACCOUNT_PREFIX_LEN_BEFORE_REVISION, TAG_ACCOUNT_BALANCE_BEFORE_REVISION,
        TAG_ACCOUNT_CONTRACT_BEFORE_REVISION, TAG_STORAGE_CELL_BEFORE_REVISION,
    },
    ACCOUNT_PREFIX_LEN, TAG_ACCOUNT_BALANCE, TAG_ACCOUNT_CONTRACT, TAG_STORAGE_CELL,
};
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::{account_storage::account_info, NeonResult};

fn from_before_revision(account: &mut Account, new_tag: u8) {
    const PREFIX_BEFORE: usize = ACCOUNT_PREFIX_LEN_BEFORE_REVISION;
    const PREFIX_AFTER: usize = ACCOUNT_PREFIX_LEN;

    let data: &mut Vec<u8> = &mut account.data;

    assert!(data.len() > PREFIX_BEFORE);
    let data_len = data.len() - PREFIX_BEFORE;

    let required_len = data.len() + PREFIX_AFTER - PREFIX_BEFORE;
    data.resize(required_len, 0);

    data.copy_within(PREFIX_BEFORE..(PREFIX_BEFORE + data_len), PREFIX_AFTER);
    data[0] = new_tag;
}

pub fn update_account(program_id: &Pubkey, key: &Pubkey, account: &mut Account) -> NeonResult<()> {
    let tag = {
        let info = account_info(key, account);
        evm_loader::account::tag(program_id, &info)?
    };

    match tag {
        TAG_ACCOUNT_BALANCE_BEFORE_REVISION => {
            from_before_revision(account, TAG_ACCOUNT_BALANCE);
        }
        TAG_ACCOUNT_CONTRACT_BEFORE_REVISION => {
            from_before_revision(account, TAG_ACCOUNT_CONTRACT);
        }
        TAG_STORAGE_CELL_BEFORE_REVISION => {
            from_before_revision(account, TAG_STORAGE_CELL);
        }
        _ => {}
    }

    Ok(())
}
