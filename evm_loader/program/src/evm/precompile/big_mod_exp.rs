use ethnum::U256;

#[must_use]
pub fn big_mod_exp(input: &[u8]) -> Vec<u8> {
    if input.len() < 96 {
        return vec![];
    }

    let (base_len, rest) = input.split_at(32);
    let (exp_len, rest) = rest.split_at(32);
    let (mod_len, rest) = rest.split_at(32);

    let Ok(base_len) = U256::from_be_bytes(base_len.try_into().unwrap()).try_into() else {
        return vec![];
    };
    let Ok(exp_len) = U256::from_be_bytes(exp_len.try_into().unwrap()).try_into() else {
        return vec![];
    };
    let Ok(mod_len) = U256::from_be_bytes(mod_len.try_into().unwrap()).try_into() else {
        return vec![];
    };

    if base_len == 0 && mod_len == 0 {
        return vec![0; 32];
    }

    let (base_val, rest) = rest.split_at(base_len);
    let (exp_val, rest) = rest.split_at(exp_len);
    let (mod_val, _) = rest.split_at(mod_len);

    solana_program::big_mod_exp::big_mod_exp(base_val, exp_val, mod_val)
}
