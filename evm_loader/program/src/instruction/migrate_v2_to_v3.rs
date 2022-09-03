use evm::Valids;
use solana_program::{pubkey, system_instruction};
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::{MAX_PERMITTED_DATA_INCREASE, ProgramResult};
use solana_program::program::invoke;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program::sysvar::Sysvar;

use crate::account::{AccountData, ether_account, ether_contract, EthereumAccount, Operator, Packable, program};
use crate::account::ether_contract::Extension;
use crate::error::EvmLoaderError;

const OPERATOR_PUBKEY: Pubkey = pubkey!("6sXBjtBYNbUCKFq3CuAg7LHw9DJCvXujRUEFgK9TuzKx");

type EthereumAccountV2<'a> = AccountData<'a, ether_account::DataV2>;
type EthereumContractV2<'a> = AccountData<'a, ether_contract::DataV2, ether_contract::Extension<'a>>;

struct Accounts<'a> {
    operator: Operator<'a>,
    system_program: program::System<'a>,
    ether_account: &'a AccountInfo<'a>,
    ether_contract: Option<&'a AccountInfo<'a>>,
}

impl<'a> Accounts<'a> {
    fn from_slice(accounts: &'a [AccountInfo<'a>]) -> Result<Self, ProgramError> {
        Ok(Self {
            operator: unsafe { Operator::from_account_not_whitelisted(&accounts[0])? },
            system_program: program::System::from(&accounts[1]),
            ether_account: &accounts[2],
            ether_contract: if accounts.len() > 3 { Some(&accounts[3]) } else { None },
        })
    }
}

pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    _instruction: &[u8],
) -> ProgramResult {
    solana_program::msg!("Instruction: Convert account from V02 to V03");

    let accounts = Accounts::from_slice(accounts)?;
    validate(program_id, &accounts)?;
    execute(program_id, &accounts)
}

fn validate(program_id: &Pubkey, accounts: &Accounts) -> ProgramResult {
    if accounts.operator.key != &OPERATOR_PUBKEY {
        return Err!(
            EvmLoaderError::UnauthorizedOperator.into();
            "Account {} - expected authorized operator",
            accounts.operator.key
        );
    }

    validate_account(program_id, accounts.ether_account, "account", EthereumAccountV2::TAG, EthereumAccountV2::SIZE)?;
    if let Some(contract) = &accounts.ether_contract {
        validate_account(program_id, contract, "contract", EthereumContractV2::TAG, EthereumContractV2::SIZE)?;
    }

    Ok(())
}

fn validate_account(
    program_id: &Pubkey,
    account: &AccountInfo,
    entity: &'static str,
    expected_tag: u8,
    minimal_size: usize,
) -> ProgramResult {
    if account.owner != program_id {
        return Err!(
            ProgramError::InvalidArgument;
            "Account {} for ether {} - expected program owned",
            account.key,
            entity
        );
    }

    let account_data_len = account.data_len();
    if account_data_len < minimal_size {
        return Err!(
            ProgramError::InvalidArgument;
            "Size ({}) of account {} for ether account must be at least {} bytes",
            account_data_len,
            account.key,
            minimal_size
        );
    }

    let account_tag = account.data.borrow()[0];
    if account_tag != expected_tag {
        return Err!(
            ProgramError::InvalidArgument;
            "Expected tag {} for account {}, actual tag is {}",
            expected_tag,
            account.key,
            account_tag
        );
    }

    Ok(())
}

fn execute(program_id: &Pubkey, accounts: &Accounts) -> ProgramResult {
    let space_needed = EthereumAccount::SIZE + if let Some(contract) = accounts.ether_contract {
        let contract_v2 = EthereumContractV2::from_account(program_id, contract)?;
        EthereumAccount::SIZE + Extension::size_needed_v3(contract_v2.code_size as usize, None)
    } else {
        0
    };

    let space_current = accounts.ether_account.data_len();
    let rent = Rent::get()?;
    let lamports_needed = rent.minimum_balance(space_needed);
    if space_current < space_needed &&
        !extend_ether_account_space(accounts, space_current, space_needed, lamports_needed)?
    {
        solana_program::msg!(
            "Account reallocation will be continued on the next step(s): {}",
            accounts.ether_account.key
        );

        return Ok(());
    }

    let data = {
        let (code_size, generation) = if let Some(contract) = accounts.ether_contract {
            let contract_v2 = EthereumContractV2::from_account(program_id, contract)?;
            (contract_v2.code_size, contract_v2.generation)
        } else {
            (0, 0)
        };

        let account_v2 = EthereumAccountV2::from_account(program_id, accounts.ether_account)?;

        ether_account::Data {
            address: account_v2.address,
            bump_seed: account_v2.bump_seed,
            trx_count: account_v2.trx_count,
            balance: account_v2.balance,
            rw_blocked: account_v2.rw_blocked,
            ro_blocked_count: account_v2.ro_blocked_count,
            generation,
            code_size,
        }
    };

    {
        let mut data_dst = accounts.ether_account.data.borrow_mut();
        data_dst[0] = EthereumAccount::TAG;
        data.pack(&mut data_dst[1..]);

        let valids_len = Valids::size_needed(data.code_size as usize);
        if let Some(contract_v2_info) = accounts.ether_contract {
            let contract_v2_data = EthereumContractV2::from_account(
                program_id,
                contract_v2_info,
            )?;

            let extension_dst = &mut data_dst[EthereumAccount::SIZE..];
            extension_dst[..data.code_size as usize].copy_from_slice(&contract_v2_data.extension.code);
            extension_dst[data.code_size as usize..][..valids_len]
                .copy_from_slice(&contract_v2_data.extension.valids[..valids_len]);
            extension_dst[data.code_size as usize..][valids_len..][..contract_v2_data.extension.storage.len()]
                .copy_from_slice(&contract_v2_data.extension.storage);

            **accounts.operator.lamports.borrow_mut() += contract_v2_info.lamports();
            **contract_v2_info.lamports.borrow_mut() = 0;
        }
    }

    if space_current > space_needed {
        accounts.ether_account.realloc(space_needed, false)?;
        let excessive_lamports = accounts.ether_account.lamports().saturating_sub(lamports_needed);
        **accounts.ether_account.lamports.borrow_mut() -= excessive_lamports;
        **accounts.operator.lamports.borrow_mut() += excessive_lamports;
    }

    Ok(())
}

fn extend_ether_account_space(
    accounts: &Accounts,
    space_current: usize,
    space_needed: usize,
    lamports_needed: u64,
) -> Result<bool, ProgramError> {
    solana_program::msg!(
        "Resizing account (space_current = {}, space_needed = {})",
        space_current,
        space_needed
    );

    let lamports_current = accounts.ether_account.lamports();
    if lamports_current < lamports_needed {
        invoke(
            &system_instruction::transfer(
                accounts.operator.key,
                accounts.ether_account.key,
                lamports_needed - lamports_current,
            ),
            &[
                (*accounts.operator.info).clone(),
                accounts.ether_account.clone(),
                (*accounts.system_program).clone(),
            ],
        )?;
    }

    let max_possible_space_per_instruction = space_needed
        .min(space_current + MAX_PERMITTED_DATA_INCREASE);
    accounts.ether_account.realloc(max_possible_space_per_instruction, false)?;

    Ok(max_possible_space_per_instruction >= space_needed)
}
