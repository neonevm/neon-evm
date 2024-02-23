use crate::account::legacy::ACCOUNT_PREFIX_LEN_BEFORE_REVISION;
use solana_program::account_info::AccountInfo;
use solana_program::rent::Rent;

use crate::account::program::System;
use crate::account::Operator;
use crate::account::ACCOUNT_PREFIX_LEN;
use crate::error::Result;

pub fn from_before_revision<'a>(
    account: &AccountInfo<'a>,
    new_tag: u8,
    operator: &Operator<'a>,
    system: &System<'a>,
    rent: &Rent,
) -> Result<()> {
    const PREFIX_BEFORE: usize = ACCOUNT_PREFIX_LEN_BEFORE_REVISION;
    const PREFIX_AFTER: usize = ACCOUNT_PREFIX_LEN;

    assert!(account.data_len() > PREFIX_BEFORE);
    let data_len = account.data_len() - PREFIX_BEFORE;

    let required_len = account.data_len() + PREFIX_AFTER - PREFIX_BEFORE;
    account.realloc(required_len, false)?;

    let minimum_balance = rent.minimum_balance(required_len);
    if account.lamports() < minimum_balance {
        let required_lamports = minimum_balance - account.lamports();
        system.transfer(operator, account, required_lamports)?;
    }

    let mut account_data = account.try_borrow_mut_data()?;
    account_data.copy_within(PREFIX_BEFORE..(PREFIX_BEFORE + data_len), PREFIX_AFTER);
    account_data[0] = new_tag;

    Ok(())
}
