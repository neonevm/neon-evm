// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of OpenEthereum.

// OpenEthereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// OpenEthereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with OpenEthereum.  If not, see <http://www.gnu.org/licenses/>.

//! Account system expressed in Plain Old Data.

use crate::types::Bytes;
// use ethereum_types::{H256, U256};
use evm_loader::{H256, U256};

//use itertools::Itertools;
use crate::types::ec::account_diff::*;
use itertools::Itertools;
use rustc_hex::ToHex;
use serde::{Serialize, Serializer};
use std::{collections::BTreeMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// An account, expressed as Plain-Old-Data (hence the name).
/// Does not have a DB overlay cache, code hash or anything like that.
pub struct PodAccount {
    /// The balance of the account.
    pub balance: U256,
    /// The nonce of the account.
    pub nonce: U256,
    #[serde(serialize_with = "opt_bytes_to_hex")]
    /// The code of the account or `None` in the special case that it is unknown.
    pub code: Option<Bytes>,
    /// The storage of the account.
    pub storage: BTreeMap<H256, H256>,
}

fn opt_bytes_to_hex<S>(opt_bytes: &Option<Bytes>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(&format_args!(
        "0x{}",
        opt_bytes.as_ref().map_or("".to_string(), |b| b.to_hex())
    ))
}
/*
impl fmt::Display for PodAccount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(bal={}; nonce={}; code={} bytes, #{}; storage={} items)",
            self.balance,
            self.nonce,
            self.code.as_ref().map_or(0, |c| c.len()),
            self.code.as_ref().map_or_else(H256::default, |c| keccak(c)),
            self.storage.len(),
        )
    }
}
*/

/// Determine difference between two optionally existant `Account`s. Returns None
/// if they are the same.
pub fn diff_pod(pre: Option<&PodAccount>, post: Option<&PodAccount>) -> Option<AccountDiff> {
    match (pre, post) {
		(None, Some(x)) => Some(AccountDiff {
			balance: Diff::Born(x.balance),
			nonce: Diff::Born(x.nonce),
			code: Diff::Born(x.code.as_ref().expect("account is newly created; newly created accounts must be given code; all caches should remain in place; qed").clone()),
			storage: x.storage.iter().map(|(k, v)| (k.clone(), Diff::Born(v.clone()))).collect(),
		}),
		(Some(x), None) => Some(AccountDiff {
			balance: Diff::Died(x.balance),
			nonce: Diff::Died(x.nonce),
			code: Diff::Died(x.code.as_ref().expect("account is deleted; only way to delete account is running SUICIDE; account must have had own code cached to make operation; all caches should remain in place; qed").clone()),
			storage: x.storage.iter().map(|(k, v)| (k.clone(), Diff::Died(v.clone()))).collect(),
		}),
		(Some(pre), Some(post)) => {
			let storage: Vec<_> = pre.storage.keys().merge(post.storage.keys())
				.filter(|k| pre.storage.get(k).unwrap_or(&H256::default()) != post.storage.get(k).unwrap_or(&H256::default()))
				.collect();
			let r = AccountDiff {
				balance: Diff::new(pre.balance, post.balance),
				nonce: Diff::new(pre.nonce, post.nonce),
				code: match (pre.code.clone(), post.code.clone()) {
					(Some(pre_code), Some(post_code)) => Diff::new(pre_code, post_code),
                                        (None, Some(post_code)) => Diff::Born(post_code),
                                        (Some(pre_code), None) => Diff::Died(pre_code),
					_ => Diff::Same,
				},
				storage: storage.into_iter().map(|k|
					(k.clone(), Diff::new(
						pre.storage.get(k).cloned().unwrap_or_else(H256::default),
						post.storage.get(k).cloned().unwrap_or_else(H256::default)
					))).collect(),
			};
			if r.balance.is_same() && r.nonce.is_same() && r.code.is_same() && r.storage.is_empty() {
				None
			} else {
				Some(r)
			}
		},
		_ => None,
	}
}
