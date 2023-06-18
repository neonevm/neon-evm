use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::cmp::Ordering;
use crate::account::{EthereumAccount, EthereumStorage};
use crate::executor::{Action, OwnedAccountInfo};
use crate::types::Address;
use ethnum::U256;
use solana_program::{ pubkey::Pubkey, slot_history::Slot };
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;

mod base;
mod apply;
mod backend;

#[derive(Debug)]
pub enum AccountOperation {
    Create {
        space: usize,
    },

    Resize {
        from: usize,
        to: usize,
    },
}

pub type AccountsOperations = Vec<(Address, AccountOperation)>;

#[derive(Debug, PartialEq, Eq)]
pub enum AccountsReadiness {
    Ready,
    NeedMoreReallocations,
}

pub struct ProgramAccountStorage<'a> {
    program_id: &'a Pubkey,
    operator: &'a Pubkey,
    clock: Clock,

    solana_accounts: BTreeMap<&'a Pubkey, &'a AccountInfo<'a>>,

    ethereum_accounts: BTreeMap<Address, EthereumAccount<'a>>,
    empty_ethereum_accounts: RefCell<BTreeSet<Address>>,

    storage_accounts: BTreeMap<(Address,U256), EthereumStorage<'a>>,
    empty_storage_accounts: RefCell<BTreeSet<(Address,U256)>>,
}

/// Account storage
/// Trait to access account info
pub trait AccountStorage {
    /// Get `NEON` token mint
    fn neon_token_mint(&self) -> &Pubkey;

    /// Get `NeonEVM` program id
    fn program_id(&self) -> &Pubkey;

    /// Get operator pubkey
    fn operator(&self) -> &Pubkey;

    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;
    /// Get block hash
    fn block_hash(&self, number: u64) -> [u8; 32];
    /// Get chain id
    fn chain_id(&self) -> u64;

    /// Check if ethereum account exists
    fn exists(&self, address: &Address) -> bool;
    /// Get account nonce
    fn nonce(&self, address: &Address) -> u64;
    /// Get account balance
    fn balance(&self, address: &Address) -> U256;

    /// Get code size
    fn code_size(&self, address: &Address) -> usize;
    /// Get code hash
    fn code_hash(&self, address: &Address) -> [u8; 32];
    /// Get code data
    fn code(&self, address: &Address) -> crate::evm::Buffer;
    /// Get contract generation
    fn generation(&self, address: &Address) -> u32;

    /// Get data from storage
    fn storage(&self, address: &Address, index: &U256) -> [u8; 32];

    /// Clone existing solana account
    fn clone_solana_account(&self, address: &Pubkey) -> OwnedAccountInfo;

    /// Map existing solana account
    fn map_solana_account<F, R>(&self, address: &Pubkey, action: F) -> R
    where
        F: FnOnce(&AccountInfo) -> R;

    /// Resolve account solana address and bump seed
    fn solana_address(&self, address: &Address) -> (Pubkey, u8) {
        address.find_solana_address(self.program_id())
    }

    /// Solana account data len
    fn solana_account_space(&self, address: &Address) -> Option<usize>;

    fn calc_accounts_operations(
        &self,
        actions: &[Action],
    ) -> AccountsOperations {
        let mut accounts = BTreeMap::new();
        for action in actions {
            let (address, code_size) = match action {
                Action::NeonTransfer { target, .. } => (target, 0),
                Action::EvmSelfDestruct { address } => (address, 0),
                Action::EvmSetCode { address, code, .. } => (address, code.len()),
                _ => continue,
            };

            let space_needed = EthereumAccount::space_needed(code_size);
            if let Some(max_size) = accounts.get_mut(&address) {
                *max_size = space_needed.max(*max_size);
                continue;
            }
            accounts.insert(address, space_needed);
        }

        accounts.into_iter()
            .filter_map(|(address, space_needed)|
                match self.solana_account_space(address) {
                    None => Some((*address, AccountOperation::Create { space: space_needed })),
                    Some(space_current) if space_current < space_needed =>
                        Some((*address, AccountOperation::Resize { from: space_current, to: space_needed })),
                    _ => None,
                }
            ).collect()
    }
}

#[must_use]
pub fn find_slot_hash(value: Slot, slot_hashes_data: &[u8]) -> [u8; 32] {
    let slot_hashes_len = u64::from_le_bytes(slot_hashes_data[..8].try_into().unwrap());

    // copy-paste from slice::binary_search
    let mut size = usize::try_from(slot_hashes_len).unwrap() - 1;
    let mut left = 0;
    let mut right = size;

    while left < right {
        let mid = left + size / 2;
        let offset = mid * 40 + 8; // +8 - the first 8 bytes for the len of vector

        let slot = u64::from_le_bytes(slot_hashes_data[offset..][..8].try_into().unwrap());
        let cmp = value.cmp(&slot);

        // The reason why we use if/else control flow rather than match
        // is because match reorders comparison operations, which is perf sensitive.
        // This is x86 asm for u8: https://rust.godbolt.org/z/8Y8Pra.
        if cmp == Ordering::Less {
            left = mid + 1;
        } else if cmp == Ordering::Greater {
            right = mid;
        } else {
            return slot_hashes_data[(offset + 8)..][..32].try_into().unwrap();
        }

        size = right - left;
    }

    generate_fake_slot_hash(value)
}

#[must_use]
pub fn generate_fake_slot_hash(slot: Slot) -> [u8; 32] {
    let slot_bytes: [u8; 8] = slot.to_be_bytes();
    let mut initial = 0;
    for b in slot_bytes {
        if b != 0 {
            break;
        }
        initial += 1;
    }
    let slot_slice = &slot_bytes[initial..];
    let slot_slice_len = slot_slice.len();
    let mut hash = [255; 32];
    hash[32 - slot_slice_len - 1] = 0;
    hash[(32 - slot_slice_len)..].copy_from_slice(slot_slice);
    hash
}

#[test]
fn test_generate_fake_slot_hash() {
    let slot = 0x46;
    let mut expected: [u8; 32] = [255; 32];
    expected[30] = 0;
    expected[31] = 0x46;
    assert_eq!(generate_fake_slot_hash(slot), expected);

    let slot = 0x3e8;
    let mut expected: [u8; 32] = [255; 32];
    expected[29] = 0;
    expected[30] = 0x03;
    expected[31] = 0xe8;
    assert_eq!(generate_fake_slot_hash(slot), expected);
}
