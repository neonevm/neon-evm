use evm::{
    backend::{Basic, Backend},
    CreateScheme, Capture, Transfer, ExitReason
};
use core::convert::Infallible;
use primitive_types::{H160, H256, U256};
use sha3::{Digest, Keccak256};
use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    instruction::{Instruction, AccountMeta},
    program::invoke_signed,
};
use std::convert::TryInto;
use arrayref::{array_ref, array_refs};
use crate::{
    solidity_account::SolidityAccount,
    utils::keccak256_digest,
};

pub trait AccountStorage {
    fn apply_to_account<U, D, F>(&self, address: &H160, d: D, f: F) -> U
    where F: FnOnce(&SolidityAccount) -> U,
          D: FnOnce() -> U;

    fn contract(&self) -> H160;
    fn origin(&self) -> H160;
    fn block_number(&self) -> U256;
    fn block_timestamp(&self) -> U256;

    fn get_account_solana_address(&self, address: &H160) -> Option<Pubkey> { self.apply_to_account(address, || None, |account| Some(account.get_solana_address())) }
    fn exists(&self, address: &H160) -> bool { self.apply_to_account(address, || false, |_| true) }
    fn basic(&self, address: &H160) -> Basic { self.apply_to_account(address, || Basic{balance: U256::zero(), nonce: U256::zero()}, |account| account.basic()) }
    fn code_hash(&self, address: &H160) -> H256 { self.apply_to_account(address, || keccak256_digest(&[]) , |account| account.code_hash()) }
    fn code_size(&self, address: &H160) -> usize { self.apply_to_account(address, || 0, |account| account.code_size()) }
    fn code(&self, address: &H160) -> Vec<u8> { self.apply_to_account(address, || Vec::new(), |account| account.get_code()) }
    fn storage(&self, address: &H160, index: &H256) -> H256 { self.apply_to_account(address, || H256::default(), |account| account.get_storage(index)) }
    fn seeds(&self, address: &H160) -> Option<(H160, u8)> {self.apply_to_account(&address, || None, |account| Some(account.get_seeds())) }
}

pub struct SolanaBackend<'a, 's, S> {
    account_storage: &'s S,
    account_infos: Option<&'a [AccountInfo<'a>]>,
}

impl<'a, 's, S> SolanaBackend<'a, 's, S> where S: AccountStorage {
    pub fn new(account_storage: &'s S, account_infos: Option<&'a [AccountInfo<'a>]>) -> Self {
        debug_print!("backend::new"); 
        Self { account_storage, account_infos }
    }

    fn is_solana_address(&self, code_address: &H160) -> bool {
        *code_address == Self::system_account()
    }

    fn is_ecrecover_address(&self, code_address: &H160) -> bool {
        *code_address == Self::system_account_ecrecover()
    }

    pub fn system_account() -> H160 {
        H160::from_slice(&[0xffu8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8])
    }

    pub fn system_account_ecrecover() -> H160 {
        H160::from_slice(&[0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0x01u8])
    }

    pub fn call_inner_ecrecover(&self,
        code_address: H160,
        _transfer: Option<Transfer>,
        input: Vec<u8>,
        _target_gas: Option<usize>,
        _is_static: bool,
        _take_l64: bool,
        _take_stipend: bool,
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        debug_print!("ecrecover");
        debug_print!("input: {}", &hex::encode(&input));
    
        if input.len() != 128 {
            return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 20])));
        }

        let data = array_ref![input, 0, 128];
        let (msg, v, sig) = array_refs![data, 32, 32, 64];
        let message = secp256k1::Message::parse(&msg);
        let v = U256::from(v).as_u32() as u8;
        let signature = secp256k1::Signature::parse(&sig);
        let recoveryId = match secp256k1::RecoveryId::parse_rpc(v) {
            Ok(value) => value,
            Err(_) => return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 20])))
        };

        let public_key = match secp256k1::recover(&message, &signature, &recoveryId) {
            Ok(value) => value,
            Err(_) => return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 20])))
        };

        let mut address = Keccak256::digest(&public_key.serialize()[1..]);
        for i in 0..12 { address[i] = 0 }
        debug_print!("{}", &hex::encode(&address));

        return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), address.to_vec())));
    }
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
    fn block_gas_limit(&self) -> U256 { U256::zero() }
    fn chain_id(&self) -> U256 { U256::from(111) }

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
    fn storage(&self, address: H160, index: H256) -> H256 {
        self.account_storage.storage(&address, &index)
    }

    fn create(&self, _scheme: &CreateScheme, _address: &H160) {
        if let CreateScheme::Create2 {caller, code_hash, salt} = _scheme {
            debug_print!("CreateScheme2 {} from {} {} {} {}", &hex::encode(_address), &hex::encode(caller), &hex::encode(code_hash), &hex::encode(salt), "" /*dummy arg for use correct message function*/);
        } else {
            debug_print!("Call create");
        }
    /*    let account = if let CreateScheme::Create2{salt,..} = scheme
                {Pubkey::new(&salt.to_fixed_bytes())} else {Pubkey::default()};
        self.add_alias(address, &account);*/
    }

    fn call_inner(&self,
        code_address: H160,
        _transfer: Option<Transfer>,
        input: Vec<u8>,
        _target_gas: Option<usize>,
        _is_static: bool,
        _take_l64: bool,
        _take_stipend: bool,
    ) -> Option<Capture<(ExitReason, Vec<u8>), Infallible>> {
        if self.is_ecrecover_address(&code_address) {
            return self.call_inner_ecrecover(code_address, _transfer, input, _target_gas, _is_static, _take_l64, _take_stipend);
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
                    let pubkey = if translate[0] != 0 {
                        match self.account_storage.get_account_solana_address(&H160::from_slice(&pubkey[12..])) {
                            Some(key) => key.clone(),
                            None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                        }
                    } else {
                        Pubkey::new(pubkey)
                    };
                    accounts.push(AccountMeta {
                        is_signer: signer[0] != 0,
                        is_writable: writable[0] != 0,
                        pubkey: pubkey,
                    });
                    debug_print!("Acc: {}", pubkey);
                };
        
                let (_, input) = input.split_at(35 * acc_length as usize);
                debug_print!("{}", &hex::encode(&input));

                let (contract_eth, contract_nonce) = self.account_storage.seeds(&self.account_storage.contract()).unwrap();   // do_call already check existence of Ethereum account with such index
                let contract_seeds = [contract_eth.as_bytes(), &[contract_nonce]];

                debug_print!("account_infos");
                for info in self.account_infos.unwrap() {
                    debug_print!("  {}", info.key);
                };
                let result : solana_program::entrypoint::ProgramResult;
                match self.account_storage.seeds(&self.account_storage.origin()) {
                    Some((sender_eth, sender_nonce)) => {
                        let sender_seeds = [sender_eth.as_bytes(), &[sender_nonce]];
                        result = invoke_signed(
                            &Instruction{program_id, accounts: accounts, data: input.to_vec()},
                            &self.account_infos.unwrap(), &[&sender_seeds[..], &contract_seeds[..]]
                        );

                    }
                    None => {
                        result = invoke_signed(
                            &Instruction{program_id, accounts: accounts, data: input.to_vec()},
                            &self.account_infos.unwrap(), &[&contract_seeds[..]]
                        );
                    }
                }
                if let Err(err) = result {
                    debug_print!("result: {}", err);
                    return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
                };
                return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Stopped), Vec::new())));
            },
            1 => {
                let data = array_ref![input, 0, 66];
                let (tr_base, tr_owner, base, owner) = array_refs![data, 1, 1, 32, 32];

                let base = if tr_base[0] != 0 {
                    match self.account_storage.get_account_solana_address(&H160::from_slice(&base[12..])) {
                        Some(key) => key.clone(),
                        None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                    }
                } else {Pubkey::new(base)};

                let owner = if tr_owner[0] != 0 {
                    match self.account_storage.get_account_solana_address(&H160::from_slice(&owner[12..])) {
                        Some(key) => key.clone(),
                        None => { return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))); },
                    }
                } else {Pubkey::new(owner)};

                let (_, seed) = input.split_at(66);
                let seed = if let Ok(seed) = std::str::from_utf8(&seed) {seed}
                else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

                let pubkey = if let Ok(pubkey) = Pubkey::create_with_seed(&base, seed.into(), &owner) {pubkey}
                else {return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));};

                debug_print!("result: {}", &hex::encode(pubkey.as_ref()));
                return Some(Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), pubkey.as_ref().to_vec())));
            },
            _ => {
                return Some(Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new())));
            }
        }
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use solana_sdk::{
        account::Account,
        account_info::{AccountInfo},
        pubkey::Pubkey,
    };
    use evm::executor::StackExecutor;

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
            v.extend_from_slice(&0x893d20e8u32.to_be_bytes());
            v
        }
    
        fn change_owner(address: H160) -> Vec<u8> {
            let mut v = Vec::new();
            v.extend_from_slice(&0xa6f9dae1u32.to_be_bytes());
            v.extend_from_slice(&[0u8;12]);
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
        let owner = Pubkey::new_rand();
        let mut accounts = Vec::new();

        for i in 0..4 {
            accounts.push( (
                    Pubkey::new_rand(), i == 0,
                    Account::new(((i+2)*1000) as u64, 10*1024, &owner)
                ) );
        }
        accounts.push((Pubkey::new_rand(), false, Account::new(1234u64, 0, &owner)));
        accounts.push((Pubkey::new_rand(), false, Account::new(5423u64, 1024, &Pubkey::new_rand())));
        accounts.push((Pubkey::new_rand(), false, Account::new(1234u64, 0, &Pubkey::new_rand())));

        for acc in &accounts {println!("{:x?}", acc);};

        let mut infos = Vec::new();
        for acc in &mut accounts {
            infos.push(AccountInfo::from((&acc.0, acc.1, &mut acc.2)));
        }

        let mut backend = SolanaBackend::new(&owner, &infos[..]).unwrap();

        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);

        assert_eq!(backend.exists(solidity_address(&owner)), false);
        assert_eq!(backend.exists(solidity_address(infos[1].key)), true);

        let creator = solidity_address(infos[1].key);
        println!("Creator: {:?}", creator);
        executor.deposit(creator, U256::exp10(18));

        let contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&TestContract::code()), salt: infos[0].key.to_bytes().into()});
        let exit_reason = executor.transact_create2(creator, U256::zero(), TestContract::code(), infos[0].key.to_bytes().into(), usize::max_value());
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        let (applies, logs) = executor.deconstruct();

//        backend.add_account(contract, &infos[0]);
        let apply_result = backend.apply(applies, logs, false);
        println!("Apply result: {:?}", apply_result);

        println!();
//        let mut backend = SolanaBackend::new(&infos).unwrap();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
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
                creator, contract, U256::zero(), TestContract::get_owner(), usize::max_value());
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
        let owner = Pubkey::new_rand();
        let mut accounts = Vec::new();

        for i in 0..4 {
            accounts.push( (
                    Pubkey::new_rand(), i == 0,
                    Account::new(((i+2)*1000) as u64, 10*1024, &owner)
                ) );
        }
        accounts.push((Pubkey::new_rand(), false, Account::new(1234u64, 0, &owner)));
        accounts.push((Pubkey::new_rand(), false, Account::new(5423u64, 1024, &Pubkey::new_rand())));
        accounts.push((Pubkey::new_rand(), false, Account::new(1234u64, 0, &Pubkey::new_rand())));

        for acc in &accounts {println!("{:x?}", acc);};

        let mut infos = Vec::new();
        for acc in &mut accounts {
            infos.push(AccountInfo::from((&acc.0, acc.1, &mut acc.2)));
        }

        let mut backend = SolanaBackend::new(&owner, &infos[..]).unwrap();

        let config = evm::Config::istanbul();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);

        assert_eq!(backend.exists(solidity_address(&owner)), false);
        assert_eq!(backend.exists(solidity_address(infos[1].key)), true);

        let creator = solidity_address(infos[1].key);
        println!("Creator: {:?}", creator);
        executor.deposit(creator, U256::exp10(18));

        let contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&ERC20Contract::wrapper_code()), salt: infos[0].key.to_bytes().into()});
        let exit_reason = executor.transact_create2(creator, U256::zero(), ERC20Contract::wrapper_code(), infos[0].key.to_bytes().into(), usize::max_value());
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        contract = executor.create_address(CreateScheme::Create2{caller: creator, code_hash: keccak256_digest(&ERC20Contract::code()), salt: infos[0].key.to_bytes().into()});
        exit_reason = executor.transact_create2(creator, U256::zero(), ERC20Contract::code(), infos[0].key.to_bytes().into(), usize::max_value());
        println!("Create contract {:?}: {:?}", contract, exit_reason);

        let (applies, logs) = executor.deconstruct();

//        backend.add_account(contract, &infos[0]);
        let apply_result = backend.apply(applies, logs, false);
        println!("Apply result: {:?}", apply_result);

        println!();
//        let mut backend = SolanaBackend::new(&infos).unwrap();
        let mut executor = StackExecutor::new(&backend, usize::max_value(), &config);
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
                creator, contract, U256::zero(), ERC20Contract::donate(), usize::max_value());
        println!("Call: {:?}, {}", exit_reason, hex::encode(&result));

        let (exit_reason, result) = executor.transact_call(
                creator, contract, U256::zero(), ERC20Contract::donateFrom(), usize::max_value());
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
