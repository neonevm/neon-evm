use std::cell::Ref;

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
use crate::account::ether_contract::ContractData;
use crate::error::EvmLoaderError;

const OPERATOR_PUBKEY: Pubkey = pubkey!("6sXBjtBYNbUCKFq3CuAg7LHw9DJCvXujRUEFgK9TuzKx");

type EthereumAccountV2<'a> = AccountData<'a, ether_account::DataV2>;

#[deprecated]
#[derive(Debug)]
pub struct ExtensionV2<'a> {
    pub code: Ref<'a, [u8]>,
    pub valids: Ref<'a, [u8]>,
    pub storage: Ref<'a, [u8]>,
}

impl<'a> ExtensionV2<'a> {
    fn unpack(data: &ether_contract::DataV2, remaining: Ref<'a, [u8]>) -> Self {
        let code_size = data.code_size as usize;
        let valids_size = (code_size / 8) + 1;

        let (code, rest) = Ref::map_split(remaining, |r| r.split_at(code_size));
        let (valids, storage) = Ref::map_split(rest, |r| r.split_at(valids_size));

        Self { code, valids, storage }
    }
}

struct EthereumContractV2<'a> {
    data: ether_contract::DataV2,
    extension: ExtensionV2<'a>,
}

impl<'a> EthereumContractV2<'a> {
    pub fn from_account(program_id: &Pubkey, info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if info.owner != program_id {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected program owned", info.key);
        }

        if info.data_len() < 1 + ether_contract::DataV2::SIZE {
            return Err!(
                ProgramError::InvalidAccountData;
                "Account {} - invalid data len, expected = {} found = {}",
                info.key,
                ether_contract::DataV2::SIZE,
                info.data_len()
            );
        }

        let account_data = info.try_borrow_data()?;

        let (tag, bytes) = Ref::map_split(account_data, |d| d.split_first().expect("data is not empty"));
        let (data, remaining) = Ref::map_split(bytes, |d| d.split_at(ether_contract::DataV2::SIZE));

        if *tag != ether_contract::DataV2::TAG {
            return Err!(ProgramError::InvalidAccountData; "Account {} - invalid tag, expected = {} found = {}", info.key, ether_contract::DataV2::TAG, tag);
        }

        let data = ether_contract::DataV2::unpack(&data);
        let extension = ExtensionV2::unpack(&data, remaining);

        Ok(Self { data, extension })
    }
}

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
        validate_account(program_id, contract, "contract", ether_contract::DataV2::TAG, ether_contract::DataV2::SIZE)?;
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
    let (space_needed, code_size, generation) = if let Some(contract) = accounts.ether_contract {
        let contract_v2 = EthereumContractV2::from_account(program_id, contract)?;
        (
            EthereumAccount::space_needed(contract_v2.data.code_size as usize),
            contract_v2.data.code_size,
            contract_v2.data.generation,
        )
    } else {
        (EthereumAccount::SIZE, 0, 0)
    };

    let mut space_current = accounts.ether_account.data_len();
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

    space_current = accounts.ether_account.data_len();

    assert!(space_current >= space_needed);

    let data = {
        let account_v2 = EthereumAccountV2::from_account(program_id, accounts.ether_account)?;

        ether_account::Data {
            address: account_v2.address,
            bump_seed: account_v2.bump_seed,
            trx_count: account_v2.trx_count,
            balance: account_v2.balance,
            generation,
            code_size,
            rw_blocked: account_v2.rw_blocked,
        }
    };

    {
        let data_dst = &mut accounts.ether_account.data.borrow_mut()[..space_needed];
        data_dst[0] = EthereumAccount::TAG;
        data.pack(&mut data_dst[1..]);

        if let Some(contract_v2_info) = accounts.ether_contract {
            let contract_v2 = EthereumContractV2::from_account(
                program_id,
                contract_v2_info,
            )?;

            let valids_len = Valids::size_needed(code_size as usize);

            solana_program::msg!(
                "Copying contract data. code_size = {}, code.len() = {}, valids: (actual len = {}, needed = {}), storage.len() = {}",
                contract_v2.data.code_size,
                contract_v2.extension.code.len(),
                contract_v2.extension.valids.len(),
                valids_len,
                contract_v2.extension.storage.len(),
            );

            assert_eq!(contract_v2.extension.code.len(), code_size as usize);
            assert!(
                valids_len == contract_v2.extension.valids.len() ||
                    valids_len == contract_v2.extension.valids.len() - 1
            );

            if code_size > 0 {
                assert!(contract_v2.extension.storage.len() >= ContractData::INTERNAL_STORAGE_SIZE);
                let extension_dst = &mut data_dst[EthereumAccount::SIZE..];
                extension_dst[..code_size as usize]
                    .copy_from_slice(&contract_v2.extension.code);
                extension_dst[code_size as usize..][..valids_len]
                    .copy_from_slice(&contract_v2.extension.valids[..valids_len]);
                extension_dst[code_size as usize..][valids_len..][..ContractData::INTERNAL_STORAGE_SIZE]
                    .copy_from_slice(&contract_v2.extension.storage[..ContractData::INTERNAL_STORAGE_SIZE]);
            }

            **accounts.operator.lamports.borrow_mut() += contract_v2_info.lamports();
            **contract_v2_info.lamports.borrow_mut() = 0;
        }
    }

    if space_current > space_needed {
        accounts.ether_account.realloc(space_needed, false)?;
        space_current = accounts.ether_account.data_len();
        let excessive_lamports = accounts.ether_account.lamports().saturating_sub(lamports_needed);
        **accounts.ether_account.lamports.borrow_mut() -= excessive_lamports;
        **accounts.operator.lamports.borrow_mut() += excessive_lamports;
    }

    assert_eq!(space_current, space_needed);

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
