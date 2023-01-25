
#[must_use]
pub fn big_mod_exp(
    _input: &[u8]
) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()

    /*
    use num_bigint::BigUint;
    use num_traits::{One, Zero};
    debug_print!("big_mod_exp");
    debug_print!("input: {}", &hex::encode(&input));

    if input.len() < 96 {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };

    let (base_len, rest) = input.split_at(32);
    let (exp_len, rest) = rest.split_at(32);
    let (mod_len, rest) = rest.split_at(32);

    let base_len: usize = match U256::from_big_endian(base_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };
    let exp_len: usize = match U256::from_big_endian(exp_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };
    let mod_len: usize = match U256::from_big_endian(mod_len).try_into() {
        Ok(value) => value,
        Err(_) => return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0; 0]))
    };

    if base_len == 0 && mod_len == 0 {
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), vec![0_u8; 32]));
    }

    let (base_val, rest) = rest.split_at(base_len);
    let (exp_val, rest) = rest.split_at(exp_len);
    let (mod_val, _rest) = rest.split_at(mod_len);

    let base_val = BigUint::from_bytes_be(base_val);
    let exp_val  = BigUint::from_bytes_be(exp_val);
    let mod_val  = BigUint::from_bytes_be(mod_val);

    if mod_val.is_zero() || mod_val.is_one() {
        let return_value = vec![0_u8; mod_len];
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), return_value));
    }

    let ret_int = base_val.modpow(&exp_val, &mod_val);
    let ret_int = ret_int.to_bytes_be();
    let mut return_value = vec![0_u8; mod_len - ret_int.len()];
    return_value.extend(ret_int);

    Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), return_value))
    */
}