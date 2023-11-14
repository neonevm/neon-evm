use crate::error::Result;
use solana_program::instruction::TRANSACTION_LEVEL_STACK_HEIGHT;
use solana_program::{account_info::AccountInfo, pubkey::Pubkey};

pub fn process<'a>(
    _program_id: &'a Pubkey,
    _accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> Result<()> {
    let stack_height = solana_program::instruction::get_stack_height();
    assert_eq!(stack_height, TRANSACTION_LEVEL_STACK_HEIGHT);

    solana_program::msg!("Instruction: Block Accounts");

    todo!();

    // let mut state = State::from_account(program_id, &accounts[0])?;
    // let operator = Operator::from_account(&accounts[1])?;

    // if &state.owner != operator.key {
    //     return Err(Error::HolderInvalidOwner(state.owner, *operator.key));
    // }

    // let mut blocked_accounts = state.read_blocked_accounts()?;
    // let mut blocked_keys: BTreeSet<Pubkey> = blocked_accounts.iter().map(|a| a.key).collect();

    // for account_info in &accounts[2..] {
    //     if blocked_keys.contains(account_info.key) {
    //         continue;
    //     }

    //     let mut meta = BlockedAccountMeta {
    //         key: *account_info.key,
    //         exists: false,
    //         is_writable: account_info.is_writable,
    //     };

    //     if let Ok(mut account) = EthereumAccount::from_account(program_id, account_info) {
    //         account.check_blocked()?;
    //         account.rw_blocked = true;

    //         meta.exists = true;
    //     }

    //     blocked_accounts.push(meta);
    //     blocked_keys.insert(*account_info.key);
    // }

    // state.update_blocked_accounts(blocked_accounts.into_iter())
}
