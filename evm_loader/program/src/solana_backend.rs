//! Solana Backend for rust evm
use evm::{
    backend::{Basic, Backend},
    CreateScheme, Capture, Transfer, ExitReason,
    H160, H256, U256
};
use core::convert::Infallible;
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
    entrypoint::ProgramResult,
};
use std::convert::TryInto;
use arrayref::{array_ref, array_refs};
use crate::{
    solidity_account::SolidityAccount,
    utils::{keccak256_h256, keccak256_h256_v, keccak256_digest},
};

/// Account storage
/// Trait to access account info
#[allow(clippy::redundant_closure_for_method_calls)]
pub trait AccountStorage {
    /// Apply function to given account
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U;

    /// Get contract address
    fn contract(&self) -> H160;
    /// Get caller address
    fn origin(&self) -> H160;
    /// Get block number
    fn block_number(&self) -> U256;
    /// Get block timestamp
    fn block_timestamp(&self) -> U256;

    /// Get solana address for given ethereum account
    fn get_account_solana_address(&self, address: &H160) -> Option<Pubkey> { self.apply_to_account(address, || None, |account| Some(account.get_solana_address())) }
    /// Check if ethereum account exists
    fn exists(&self, address: &H160) -> bool { self.apply_to_account(address, || false, |_| true) }
    /// Get account basic info (balance and nonce)
    fn basic(&self, address: &H160) -> Basic { self.apply_to_account(address, || Basic{balance: U256::zero(), nonce: U256::zero()}, |account| account.basic()) }
    /// Get code hash
    fn code_hash(&self, address: &H160) -> H256 { self.apply_to_account(address, || keccak256_h256(&[]) , |account| account.code_hash()) }
    /// Get code size
    fn code_size(&self, address: &H160) -> usize { self.apply_to_account(address, || 0, |account| account.code_size()) }
    /// Get code data
    fn code(&self, address: &H160) -> Vec<u8> { self.apply_to_account(address, Vec::new, |account| account.get_code()) }
    /// Get valids data
    fn valids(&self, address: &H160) -> Vec<u8> { self.apply_to_account(address, Vec::new, |account| account.get_valids()) }
    /// Get data from storage
    fn storage(&self, address: &H160, index: &U256) -> U256 { self.apply_to_account(address, U256::zero, |account| account.get_storage(index)) }
    /// Get account seeds
    fn seeds(&self, address: &H160) -> Option<(H160, u8)> {self.apply_to_account(address, || None, |account| Some(account.get_seeds())) }
    /// External call
    /// # Errors
    /// Will return `Err` if the external call returns err
    fn external_call(&self, _: &Instruction, _: &[AccountInfo]) -> ProgramResult { Ok(()) }
}

/// Solana Backend for rust evm
pub struct SolanaBackend<'a, 's, S> {
    account_storage: &'s S,
    account_infos: Option<&'a [AccountInfo<'a>]>,
}

static SYSTEM_ACCOUNT: H160 = H160([0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
static SYSTEM_ACCOUNT_ECRECOVER: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01]);
const SYSTEM_ACCOUNT_SHA_256: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x02]);
static SYSTEM_ACCOUNT_RIPEMD160: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x03]);
static SYSTEM_ACCOUNT_BLAKE2F: H160 = H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09]);

impl<'a, 's, S> SolanaBackend<'a, 's, S> where S: AccountStorage {
    /// Create `SolanaBackend`
    pub fn new(account_storage: &'s S, account_infos: Option<&'a [AccountInfo<'a>]>) -> Self {
        debug_print!("backend::new"); 
        Self { account_storage, account_infos }
    }

    #[allow(clippy::unused_self)]
    fn is_solana_address(&self, code_address: &H160) -> bool {
        *code_address == SYSTEM_ACCOUNT
    }

    /// Is system address
    #[must_use]
    pub fn is_system_address(address: &H160) -> bool {
        *address == SYSTEM_ACCOUNT
        || *address == SYSTEM_ACCOUNT_ECRECOVER
        || *address == SYSTEM_ACCOUNT_SHA_256
        || *address == SYSTEM_ACCOUNT_RIPEMD160
        || *address == SYSTEM_ACCOUNT_BLAKE2F
    }

    /// Call inner `ecrecover`
    #[must_use]
    pub fn call_inner_ecrecover(
        input: &[u8],
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        debug_print!("ecrecover");
        debug_print!("input: {}", &hex::encode(&input));

        if input.len() != 128 {
            return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32])));
        }

        let data = array_ref![input, 0, 128];
        let (msg, v, sig) = array_refs![data, 32, 32, 64];
        let message = secp256k1::Message::parse(msg);

        let signature = secp256k1::Signature::parse(sig);

        let v: u8 = match U256::from_big_endian(v).try_into() {
            Ok(value) => value,
            Err(_) => return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32])))
        };
        let recovery_id = match secp256k1::RecoveryId::parse_rpc(v) {
            Ok(value) => value,
            Err(_) => return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32])))
        };

        let public_key = match secp256k1::recover(&message, &signature, &recovery_id) {
            Ok(value) => value,
            Err(_) => return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 32])))
        };

        let mut address = keccak256_digest(&public_key.serialize()[1..]);
        address[0..12].fill(0);
        debug_print!("{}", &hex::encode(&address));

        Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), address)))
    }

    /// Call inner `sha256`
    #[must_use]
    pub fn call_inner_sha256(
        input: &[u8],
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {        
        use solana_program::hash::hash as sha256_digest;
        debug_print!("sha256");

        let hash = sha256_digest(input);

        debug_print!("{}", &hex::encode(hash.to_bytes()));

        Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), hash.to_bytes().to_vec())))
    }

    /// Call inner `ripemd160`
    #[must_use]
    pub fn call_inner_ripemd160(
        input: &[u8],
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        use ripemd160::{Ripemd160, Digest};
        debug_print!("ripemd160");

        let mut hasher = Ripemd160::new();
        // process input message
        hasher.update(input);
        // acquire hash digest in the form of GenericArray,
        // which in this case is equivalent to [u8; 20]
        let hash_val = hasher.finalize();

        // transform to [u8; 32]
        let mut result = vec![0_u8; 12];
        result.extend(&hash_val[..]);

        debug_print!("{}", &hex::encode(&result));

        Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), result)))
    }

    /// Call inner `blake2F`
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn call_inner_blake2_f(
        input: &[u8],
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        const BLAKE2_F_ARG_LEN: usize = 213;
        debug_print!("blake2F");

        let compress = |h: &mut [u64; 8], m: [u64; 16], t: [u64; 2], f: bool, rounds: usize| {
            const SIGMA: [[usize; 16]; 10] = [
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
                [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
                [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
                [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
                [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
                [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
                [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
                [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
                [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
                [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
            ];
            const IV: [u64; 8] = [
                0x6a09_e667_f3bc_c908,
                0xbb67_ae85_84ca_a73b,
                0x3c6e_f372_fe94_f82b,
                0xa54f_f53a_5f1d_36f1,
                0x510e_527f_ade6_82d1,
                0x9b05_688c_2b3e_6c1f,
                0x1f83_d9ab_fb41_bd6b,
                0x5be0_cd19_137e_2179,
            ];
            let g = |v: &mut [u64], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64| {
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
                v[d] = (v[d] ^ v[a]).rotate_right(32);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(24);
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
                v[d] = (v[d] ^ v[a]).rotate_right(16);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(63);
            };

            let mut v = [0_u64; 16];
            v[..h.len()].copy_from_slice(h); // First half from state.
            v[h.len()..].copy_from_slice(&IV); // Second half from IV.

            v[12] ^= t[0];
            v[13] ^= t[1];
        
            if f {
                v[14] = !v[14] // Invert all bits if the last-block-flag is set.
            }
            for i in 0..rounds {
                // Message word selection permutation for this round.
                let s = &SIGMA[i % 10];
                g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
                g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
                g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
                g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        
                g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
                g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
                g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
                g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
            }
        
            for i in 0..8 {
                h[i] ^= v[i] ^ v[i + 8];
            }
        };

        if input.len() != BLAKE2_F_ARG_LEN {
            // return Err(ExitError::Other("input length for Blake2 F precompile should be exactly 213 bytes".into()));
            return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new())))
        }

        let mut rounds_arr: [u8; 4] = Default::default();
        let (rounds_buf, input) = input.split_at(4);
        rounds_arr.copy_from_slice(rounds_buf);
        let rounds: u32 = u32::from_be_bytes(rounds_arr);

        // we use from_le_bytes below to effectively swap byte order to LE if architecture is BE

        let (h_buf, input) = input.split_at(64);
        let mut h = [0_u64; 8];
        let mut ctr = 0;
        for state_word in &mut h {
            let mut temp: [u8; 8] = Default::default();
            temp.copy_from_slice(&h_buf[(ctr * 8)..(ctr + 1) * 8]);
            *state_word = u64::from_le_bytes(temp);
            ctr += 1;
        }

        let (m_buf, input) = input.split_at(128);
        let mut m = [0_u64; 16];
        ctr = 0;
        for msg_word in &mut m {
            let mut temp: [u8; 8] = Default::default();
            temp.copy_from_slice(&m_buf[(ctr * 8)..(ctr + 1) * 8]);
            *msg_word = u64::from_le_bytes(temp);
            ctr += 1;
        }

        let mut t_0_arr: [u8; 8] = Default::default();
        let (t_0_buf, input) = input.split_at(8);
        t_0_arr.copy_from_slice(t_0_buf);
        let t_0 = u64::from_le_bytes(t_0_arr);

        let mut t_1_arr: [u8; 8] = Default::default();
        let (t_1_buf, input) = input.split_at(8);
        t_1_arr.copy_from_slice(t_1_buf);
        let t_1 = u64::from_le_bytes(t_1_arr);

        let f = if input[0] == 1 { true } else if input[0] == 0 { false } else {
            // return Err(ExitError::Other("incorrect final block indicator flag".into()))
            return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new())))
        };

        compress(&mut h, m, [t_0, t_1], f, rounds as usize);

        let mut output_buf = [0_u8; 64];
        for (i, state_word) in h.iter().enumerate() {
            output_buf[i * 8..(i + 1) * 8].copy_from_slice(&state_word.to_le_bytes());
        }

        debug_print!("{}", &hex::encode(&output_buf));

        Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output_buf.to_vec())))
    }

    /// Get chain id
    #[must_use]
    pub fn chain_id() -> U256 { U256::from(111) }
}

impl<'a, 's, S> Backend for SolanaBackend<'a, 's, S> where S: AccountStorage {
    fn gas_price(&self) -> U256 { U256::zero() }
    fn origin(&self) -> H160 { self.account_storage.origin() }
    fn block_hash(&self, _number: U256) -> H256 { H256::default() }
    fn block_number(&self) -> U256 {
        self.account_storage.block_number()
    }
    fn block_coinbase(&self) -> H160 { H160::default() }
    fn block_timestamp(&self) -> U256 {
        self.account_storage.block_timestamp()
    }
    fn block_difficulty(&self) -> U256 { U256::zero() }
    fn block_gas_limit(&self) -> U256 { U256::from(u64::MAX) }
    fn chain_id(&self) -> U256 { Self::chain_id() }

    fn exists(&self, address: H160) -> bool {
        self.account_storage.exists(&address)
    }
    fn basic(&self, address: H160) -> Basic {
        self.account_storage.basic(&address)
    }
    fn code_hash(&self, address: H160) -> H256 {
        self.account_storage.code_hash(&address)
    }
    fn code_size(&self, address: H160) -> usize {
        self.account_storage.code_size(&address)
    }
    fn code(&self, address: H160) -> Vec<u8> {
        self.account_storage.code(&address)
    }
    fn valids(&self, address: H160) -> Vec<u8> {
        self.account_storage.valids(&address)
    }
    fn storage(&self, address: H160, index: U256) -> U256 {
        self.account_storage.storage(&address, &index)
    }

    #[allow(unused_variables)]
    fn create(&self, scheme: &CreateScheme, address: &H160) {
        if let CreateScheme::Create2 {caller, code_hash, salt} = scheme {
            debug_print!("CreateScheme2 {} from {} {} {} {}", &hex::encode(address), &hex::encode(caller), &hex::encode(code_hash), &hex::encode(salt), "" /*dummy arg for use correct message function*/);
        } else {
            debug_print!("Call create");
        }
        /* let account = if let CreateScheme::Create2{salt,..} = scheme
                {Pubkey::new(&salt.to_fixed_bytes())} else {Pubkey::default()};
        self.add_alias(address, &account);*/
    }

    fn call_inner(&self,
        code_address: H160,
        _transfer: Option<Transfer>,
        input: Vec<u8>,
        _target_gas: Option<u64>,
        _is_static: bool,
        _take_l64: bool,
        _take_stipend: bool,
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        if code_address == SYSTEM_ACCOUNT_ECRECOVER {
            return Self::call_inner_ecrecover(&input);
        }
        if code_address == SYSTEM_ACCOUNT_RIPEMD160 {
            return Self::call_inner_ripemd160(&input);
        }
        if code_address == SYSTEM_ACCOUNT_SHA_256 {
            return Self::call_inner_sha256(&input);
        }
        if code_address == SYSTEM_ACCOUNT_BLAKE2F {
            return Self::call_inner_blake2_f(&input);
        }

        if !self.is_solana_address(&code_address) {
            return None;
        }

        debug_print!("Call inner");
        debug_print!("{}", &code_address.to_string());
        debug_print!("{}", &hex::encode(&input));

        let (cmd, input) = input.split_at(1);
        match cmd[0] {
            0 => {
                let (program_id, input) = input.split_at(32);
                let program_id = Pubkey::new(program_id);
        
                let (acc_length, input) = input.split_at(2);
                let acc_length = acc_length.try_into().ok().map(u16::from_be_bytes).unwrap();
                
                let mut accounts = Vec::new();
                for i in 0..acc_length {
                    let data = array_ref![input, 35*i as usize, 35];
                    let (translate, signer, writable, pubkey) = array_refs![data, 1, 1, 1, 32];
                    let pubkey = if translate[0] == 0 {
                        Pubkey::new(pubkey)
                    } else {
                        match self.account_storage.get_account_solana_address(&H160::from_slice(&pubkey[12..])) {
                            Some(key) => key,
                            None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                        }
                    };
                    accounts.push(AccountMeta {
                        is_signer: signer[0] != 0,
                        is_writable: writable[0] != 0,
                        pubkey,
                    });
                    debug_print!("Acc: {}", pubkey);
                };
        
                let (_, input) = input.split_at(35 * acc_length as usize);
                debug_print!("{}", &hex::encode(&input));

                debug_print!("account_infos[");
                #[allow(unused_variables)]
                for info in self.account_infos.unwrap() {
                    debug_print!("  {}", info.key);
                };
                debug_print!("]");

                let result = self.account_storage.external_call(
                    &Instruction { program_id, accounts, data: input.to_vec() },
                    self.account_infos.unwrap(),
                );

                debug_print!("result: {:?}", result);

                #[allow(unused_variables)]
                if let Err(err) = result {
                    debug_print!("result/err: {}", err);
                    return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
                };
                Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Stopped), Vec::new())))
            },
            1 => {
                let data = array_ref![input, 0, 66];
                let (tr_base, tr_owner, base, owner) = array_refs![data, 1, 1, 32, 32];

                let base = if tr_base[0] == 0 {
                    Pubkey::new(base)
                } else {
                    match self.account_storage.get_account_solana_address(&H160::from_slice(&base[12..])) {
                        Some(key) => key,
                        None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                    }
                };

                let owner = if tr_owner[0] == 0 {
                    Pubkey::new(owner)
                } else {
                    match self.account_storage.get_account_solana_address(&H160::from_slice(&owner[12..])) {
                        Some(key) => key,
                        None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                    }
                };

                let (_, seed) = input.split_at(66);
                let seed = if let Ok(seed) = std::str::from_utf8(seed) {seed}
                else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

                let pubkey = if let Ok(pubkey) = Pubkey::create_with_seed(&base, seed, &owner) {pubkey}
                else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

                debug_print!("result: {}", &hex::encode(pubkey.as_ref()));
                Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), pubkey.as_ref().to_vec())))
            },
            _ => {
                Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())))
            }
        }
    }

    fn keccak256_h256(&self, data: &[u8]) -> H256 {
        keccak256_h256(data)
    }

    fn keccak256_h256_v(&self, data: &[&[u8]]) -> H256 {
        keccak256_h256_v(data)
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::*;
    use solana_sdk::{
        account::Account,
        account_info::{AccountInfo},
        pubkey::Pubkey,
        program_error::ProgramError,
    };
    use evm::executor::StackExecutor;
    use std::str::FromStr;

    pub struct TestContract;
    impl TestContract {
        fn code() -> Vec<u8> {
            hex::decode("608060405234801561001057600080fd5b50336000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffff\
                         ffffffffffffffffff16600073ffffffffffffffffffffffffffffffffffffffff167f342827c97908e5e2f71151c08502a66d44b6f758e3ac2f1de95f02eb95f0a73560405160405180910390a361030e806100dc6000396000f3fe60806040526004361061002d5760003560e01c8063893d20e814610087578063a6f9dae1\
                         146100de57610082565b36610082573373ffffffffffffffffffffffffffffffffffffffff167f357b676c439b9e49b4410f8eb8680bee4223724802d8e3fd422e1756f87b475f346040518082815260200191505060405180910390a2005b600080fd5b34801561009357600080fd5b5061009c61012f565b604051808273ff\
                         ffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200191505060405180910390f35b3480156100ea57600080fd5b5061012d6004803603602081101561010157600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092\
                         9190505050610158565b005b60008060009054906101000a900473ffffffffffffffffffffffffffffffffffffffff16905090565b6000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffff\
                         ffffff161461021a576040517f08c379a00000000000000000000000000000000000000000000000000000000081526004018080602001828103825260138152602001807f43616c6c6572206973206e6f74206f776e65720000000000000000000000000081525060200191505060405180910390fd5b8073ffffffffffffff\
                         ffffffffffffffffffffffffff166000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff167f342827c97908e5e2f71151c08502a66d44b6f758e3ac2f1de95f02eb95f0a73560405160405180910390a3806000806101000a81548173ffff\
                         ffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505056fea2646970667358221220b849632806a5977f44b6046c4fe652d5d08e1bbfeec2623ad673961467e58efc64736f6c63430006060033").unwrap()
        }
    
        fn get_owner() -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&0x893d_20e8_u32.to_be_bytes());
            v
        }
    
        fn change_owner(address: H160) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&0xa6f9_dae1_u32.to_be_bytes());
            v.extend_from_slice(&[0_u8;12]);
            v.extend_from_slice(&<[u8;20]>::from(address));
            v
        }
    }
    
    pub struct ERC20Contract;
    impl ERC20Contract {
        fn wrapper_code() -> Vec<u8> {
            hex::decode("608060405273ff000000000000000000000000000000000000006000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff16021790555034801561006457600080fd5b50610ca0806100746000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c806354d5db4c1461003b578063fa432d5d14610057575b600080fd5b61005560048036036100509190810190610657565b610073565b005b610071600480360361006c91908101906105b0565b610210565b005b600060019050606060036040519080825280602002602001820160405280156100b657816020015b6100a36104ef565b81526020019060019003908161009b5790505b50905060405180608001604052806000151581526020016000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff166040516020016100ff91906108f1565b6040516020818303038152906040528152602001600115158152602001600015158152508160008151811061013057fe5b602002602001018190525060405180608001604052806001151581526020013060405160200161016091906108f1565b6040516020818303038152906040528152602001600115158152602001600015158152508160018151811061019157fe5b60200260200101819052506040518060800160405280861515815260200185815260200160001515815260200160011515815250816002815181106101d257fe5b60200260200101819052506102088183856040516020016101f492919061094f565b60405160208183030381529060405261038e565b505050505050565b60008090506060600360405190808252806020026020018201604052801561025257816020015b61023f6104ef565b8152602001906001900390816102375790505b50905060405180608001604052806000151581526020016000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1660405160200161029b91906108f1565b604051602081830303815290604052815260200160011515815260200160001515815250816000815181106102cc57fe5b602002602001018190525060405180608001604052808815158152602001878152602001600115158152602001600015158152508160018151811061030d57fe5b602002602001018190525060405180608001604052808615158152602001858152602001600015158152602001600115158152508160028151811061034e57fe5b6020026020010181905250610384818385604051602001610370929190610923565b60405160208183030381529060405261038e565b5050505050505050565b6060600060606040518060600160405280602b8152602001610c33602b9139905060608186866040516024016103c69392919061097b565b6040516020818303038152906040527ff6fb1cc3000000000000000000000000000000000000000000000000000000007bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19166020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff8381831617835250505050905060606000809054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168260405161048d919061090c565b6000604051808303816000865af19150503d80600081146104ca576040519150601f19603f3d011682016040523d82523d6000602084013e6104cf565b606091505b508092508195505050836104e257600080fd5b8094505050505092915050565b6040518060800160405280600015158152602001606081526020016000151581526020016000151581525090565b60008135905061052c81610bed565b92915050565b600082601f83011261054357600080fd5b8135610556610551826109f4565b6109c7565b9150808252602083016020830185838301111561057257600080fd5b61057d838284610b21565b50505092915050565b60008135905061059581610c04565b92915050565b6000813590506105aa81610c1b565b92915050565b600080600080600060a086880312156105c857600080fd5b60006105d68882890161051d565b955050602086013567ffffffffffffffff8111156105f357600080fd5b6105ff88828901610532565b94505060406106108882890161051d565b935050606086013567ffffffffffffffff81111561062d57600080fd5b61063988828901610532565b925050608061064a88828901610586565b9150509295509295909350565b60008060006060848603121561066c57600080fd5b600061067a8682870161051d565b935050602084013567ffffffffffffffff81111561069757600080fd5b6106a386828701610532565b92505060406106b48682870161059b565b9150509250925092565b60006106ca8383610849565b905092915050565b6106e36106de82610ab8565b610b63565b82525050565b60006106f482610a30565b6106fe8185610a69565b93508360208202850161071085610a20565b8060005b8581101561074c578484038952815161072d85826106be565b945061073883610a5c565b925060208a01995050600181019050610714565b50829750879550505050505092915050565b61076781610aca565b82525050565b600061077882610a46565b6107828185610a8b565b9350610792818560208601610b30565b61079b81610bb5565b840191505092915050565b60006107b182610a46565b6107bb8185610a9c565b93506107cb818560208601610b30565b80840191505092915050565b60006107e282610a3b565b6107ec8185610a7a565b93506107fc818560208601610b30565b61080581610bb5565b840191505092915050565b600061081b82610a51565b6108258185610aa7565b9350610835818560208601610b30565b61083e81610bb5565b840191505092915050565b6000608083016000830151610861600086018261075e565b506020830151848203602086015261087982826107d7565b915050604083015161088e604086018261075e565b5060608301516108a1606086018261075e565b508091505092915050565b6108bd6108b882610af6565b610b87565b82525050565b6108d46108cf82610b00565b610b91565b82525050565b6108eb6108e682610b14565b610ba3565b82525050565b60006108fd82846106d2565b60148201915081905092915050565b600061091882846107a6565b915081905092915050565b600061092f82856108da565b60018201915061093f82846108ac565b6020820191508190509392505050565b600061095b82856108da565b60018201915061096b82846108c3565b6008820191508190509392505050565b600060608201905081810360008301526109958186610810565b905081810360208301526109a981856106e9565b905081810360408301526109bd818461076d565b9050949350505050565b6000604051905081810181811067ffffffffffffffff821117156109ea57600080fd5b8060405250919050565b600067ffffffffffffffff821115610a0b57600080fd5b601f19601f8301169050602081019050919050565b6000819050602082019050919050565b600081519050919050565b600081519050919050565b600081519050919050565b600081519050919050565b6000602082019050919050565b600082825260208201905092915050565b600082825260208201905092915050565b600082825260208201905092915050565b600081905092915050565b600082825260208201905092915050565b6000610ac382610ad6565b9050919050565b60008115159050919050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b6000819050919050565b600067ffffffffffffffff82169050919050565b600060ff82169050919050565b82818337600083830152505050565b60005b83811015610b4e578082015181840152602081019050610b33565b83811115610b5d576000848401525b50505050565b6000610b6e82610b75565b9050919050565b6000610b8082610be0565b9050919050565b6000819050919050565b6000610b9c82610bc6565b9050919050565b6000610bae82610bd3565b9050919050565b6000601f19601f8301169050919050565b60008160c01b9050919050565b60008160f81b9050919050565b60008160601b9050919050565b610bf681610aca565b8114610c0157600080fd5b50565b610c0d81610af6565b8114610c1857600080fd5b50565b610c2481610b00565b8114610c2f57600080fd5b5056fe546f6b656e6b65675166655a79694e77414a624e62474b5046584357754276663953733632335651354441a365627a7a72315820e5121293a83e25a54f9242231e22c734eaf2d099e0cf50b0b2e55ed664f1b5626c6578706572696d656e74616cf564736f6c63430005110040").unwrap()
        }

        fn code() -> Vec<u8> {
            hex::decode("608060405234801561001057600080fd5b50610283806100206000396000f3fe608060405234801561001057600080fd5b50600436106100355760003560e01c8062362a951461003a5780637c64bbc91461007e575b600080fd5b61007c6004803603602081101561005057600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506100c2565b005b6100c06004803603602081101561009457600080fd5b81019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610174565b005b60008190508073ffffffffffffffffffffffffffffffffffffffff166354d5db4c600160056040518363ffffffff1660e01b81526004018083151515158152602001806020018367ffffffffffffffff168152602001828103825260148152602001806c010000000000000000000000008152506020019350505050600060405180830381600087803b15801561015857600080fd5b505af115801561016c573d6000803e3d6000fd5b505050505050565b60008190508073ffffffffffffffffffffffffffffffffffffffff1663fa432d5d60018060056040518463ffffffff1660e01b81526004018084151515158152602001806020018415151515815260200180602001848152602001838103835260148152602001806c02000000000000000000000000815250602001838103825260148152602001806c0100000000000000000000000081525060200195505050505050600060405180830381600087803b15801561023257600080fd5b505af1158015610246573d6000803e3d6000fd5b50505050505056fea265627a7a72315820ca2437b183207f96490f27151feae3066ef011cc1e18ae150f0ecae87100317364736f6c63430005110032").unwrap()
        }

        fn donate() -> Vec<u8> {
            hex::decode("ed88c68e").unwrap()
        }

        fn donateFrom() -> Vec<u8> {
            hex::decode("3071fbec").unwrap()
        }
    }

    #[test]
    fn test_solidity_address() -> Result<(), ProgramError> {
        use std::str::FromStr;
//        let account = Pubkey::from_str("Bfj8CF5ywavXyqkkuKSXt5AVhMgxUJgHfQsQjPc1JKzj").unwrap();
        let account = Pubkey::from_str("SysvarRent111111111111111111111111111111111").unwrap();
        let account = Pubkey::from_str("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r").unwrap();
        let sol_acc = solidity_address(&account);
        println!("{:?}", hex::encode(account.to_bytes()));
        println!("{:?}", hex::encode(sol_acc));
        Ok(())
    }

    #[test]
    fn test_solana_backend() -> Result<(), ProgramError> {
        let owner = Pubkey::new_unique();
        let mut accounts = Vec::new();

        for i in 0..4 {
            accounts.push( (
                    Pubkey::new_unique(), i == 0,
                    Account::new(((i+2)*1000) as u64, 10*1024, &owner)
                ) );
        }
        accounts.push((Pubkey::new_unique(), false, Account::new(1234u64, 0, &owner)));
        accounts.push((Pubkey::new_unique(), false, Account::new(5423u64, 1024, &Pubkey::new_unique())));
        accounts.push((Pubkey::new_unique(), false, Account::new(1234u64, 0, &Pubkey::new_unique())));

        for acc in &accounts {println!("{:x?}", acc);};

        let mut infos = Vec::new();
        for acc in &mut accounts {
            infos.push(AccountInfo::from((&acc.0, acc.1, &mut acc.2)));
        }

        let mut backend = SolanaBackend::new(&owner, Some(&infos[..]));

        let config = evm::Config::default();
        let mut executor = StackExecutor::new(&backend, u64::MAX, &config);

        assert_eq!(backend.exists(solidity_address(&owner)), false);
        assert_eq!(backend.exists(solidity_address(infos[1].key)), true);

        let creator = solidity_address(infos[1].key);
        println!("Creator: {:?}", creator);
        executor.deposit(creator, U256::exp10(18));

        let contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&TestContract::code()), salt: infos[0].key.to_bytes().into()});
        let exit_reason = executor.transact_create2(creator, U256::zero(), TestContract::code(), infos[0].key.to_bytes().into(), u64::MAX);
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        let (applies, logs) = executor.deconstruct();

//        backend.add_account(contract, &infos[0]);
        let apply_result = backend.apply(applies, logs, false);
        println!("Apply result: {:?}", apply_result);

        println!();
//        let mut backend = SolanaBackend::new(&infos).unwrap();
        let mut executor = StackExecutor::new(&backend, u64::MAX, &config);
        println!("======================================");
        println!("Contract: {:x}", contract);
        println!("{:x?}", backend.exists(contract));
        println!("{:x}", backend.code_size(contract));
        println!("code_hash {:x}", backend.code_hash(contract));
        println!("code: {:x?}", hex::encode(backend.code(contract)));
        println!("storage value: {:x}", backend.storage(contract, H256::default()));
        println!();

        println!("Creator: {:x}", creator);
        println!("code_size: {:x}", backend.code_size(creator));
        println!("code_hash: {:x}", backend.code_hash(creator));
        println!("code: {:x?}", hex::encode(backend.code(creator)));

        println!("Missing account code_size: {:x}", backend.code_size(H160::zero()));
        println!("Code_hash: {:x}", backend.code_hash(H160::zero()));
        println!("storage value: {:x}", backend.storage(H160::zero(), H256::default()));

        let (exit_reason, result) = executor.transact_call(
                creator, contract, U256::zero(), TestContract::get_owner(), u64::MAX);
        println!("Call: {:?}, {}", exit_reason, hex::encode(&result));

        let (applies, logs) = executor.deconstruct();
        backend.apply(applies, logs, false)?;
        

/*        println!();
        for acc in &accounts {
            println!("{:x?}", acc);
        }*/
        Ok(())
    }

    #[test]
    fn test_erc20_wrapper() -> Result<(), ProgramError> {
        let owner = Pubkey::new_unique();
        let mut accounts = Vec::new();

        for i in 0..4 {
            accounts.push( (
                    Pubkey::new_unique(), i == 0,
                    Account::new(((i+2)*1000) as u64, 10*1024, &owner)
                ) );
        }
        accounts.push((Pubkey::new_unique(), false, Account::new(1234u64, 0, &owner)));
        accounts.push((Pubkey::new_unique(), false, Account::new(5423u64, 1024, &Pubkey::new_unique())));
        accounts.push((Pubkey::new_unique(), false, Account::new(1234u64, 0, &Pubkey::new_unique())));

        for acc in &accounts {println!("{:x?}", acc);};

        let mut infos = Vec::new();
        for acc in &mut accounts {
            infos.push(AccountInfo::from((&acc.0, acc.1, &mut acc.2)));
        }

        let mut backend = SolanaBackend::new(&owner, &infos[..]).unwrap();

        let config = evm::Config::default();
        let mut executor = StackExecutor::new(&backend, u64::MAX, &config);

        assert_eq!(backend.exists(solidity_address(&owner)), false);
        assert_eq!(backend.exists(solidity_address(infos[1].key)), true);

        let creator = solidity_address(infos[1].key);
        println!("Creator: {:?}", creator);
        executor.deposit(creator, U256::exp10(18));

        let contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&ERC20Contract::wrapper_code()), salt: infos[0].key.to_bytes().into()});
        let exit_reason = executor.transact_create2(creator, U256::zero(), ERC20Contract::wrapper_code(), infos[0].key.to_bytes().into(), u64::MAX);
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&ERC20Contract::code()), salt: infos[0].key.to_bytes().into()});
        exit_reason = executor.transact_create2(creator, U256::zero(), ERC20Contract::code(), infos[0].key.to_bytes().into(), u64::MAX);
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        let (applies, logs) = executor.deconstruct();

//        backend.add_account(contract, &infos[0]);
        let apply_result = backend.apply(applies, logs, false);
        println!("Apply result: {:?}", apply_result);

        println!();
//        let mut backend = SolanaBackend::new(&infos).unwrap();
        let mut executor = StackExecutor::new(&backend, u64::MAX, &config);
        println!("======================================");
        println!("Contract: {:x}", contract);
        println!("{:x?}", backend.exists(contract));
        println!("{:x}", backend.code_size(contract));
        println!("code_hash {:x}", backend.code_hash(contract));
        println!("code: {:x?}", hex::encode(backend.code(contract)));
        println!("storage value: {:x}", backend.storage(contract, H256::default()));
        println!();

        println!("Creator: {:x}", creator);
        println!("code_size: {:x}", backend.code_size(creator));
        println!("code_hash: {:x}", backend.code_hash(creator));
        println!("code: {:x?}", hex::encode(backend.code(creator)));

        println!("Missing account code_size: {:x}", backend.code_size(H160::zero()));
        println!("Code_hash: {:x}", backend.code_hash(H160::zero()));
        println!("storage value: {:x}", backend.storage(H160::zero(), H256::default()));

        let (exit_reason, result) = executor.transact_call(
                creator, contract, U256::zero(), ERC20Contract::donate(), u64::MAX);
        println!("Call: {:?}, {}", exit_reason, hex::encode(&result));

        let (exit_reason, result) = executor.transact_call(
                creator, contract, U256::zero(), ERC20Contract::donateFrom(), u64::MAX);
        println!("Call: {:?}, {}", exit_reason, hex::encode(&result));

        let (applies, logs) = executor.deconstruct();
        backend.apply(applies, logs, false)?;
        

/*        println!();
        for acc in &accounts {
            println!("{:x?}", acc);
        }*/
        Ok(())
    }
}
