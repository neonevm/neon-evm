
/* Should be implemented via Solana syscall
#[must_use]
#[allow(clippy::unused_self)]
fn get_g1(
    input: &[u8]
) -> Option<G1> {
    use tbn::{AffineG1, Fq, Group, GroupError};
    if input.len() < 64 {
        return None
    }

    let (ax_slice, input) = input.split_at(32);
    let (ay_slice, _input) = input.split_at(32);

    let fq_xa = if let Ok(fq_xa) = Fq::from_slice(ax_slice) {
        fq_xa
    } else {
        debug_print!("Invalid Fq point");
        return None
    };
    let fq_ya = if let Ok(fq_ya) = Fq::from_slice(ay_slice) {
        fq_ya
    } else {
        debug_print!("Invalid Fq point");
        return None
    };

    let a : G1 = if fq_xa.is_zero() && fq_ya.is_zero() {
        G1::zero()
    } else {
        match AffineG1::new(fq_xa, fq_ya) {
            Ok(a) => a.into(),
            Err(GroupError::NotOnCurve) => {
                debug_print!("Invalid G1 point: NotOnCurve");
                return None
            },
            Err(GroupError::NotInSubgroup) => {
                debug_print!("Invalid G1 point: NotInSubgroup");
                return None
            }
        }
    };

    Some(a)
}*/

/// Call inner `bn256Add`
#[must_use]
#[allow(unused)]
pub fn bn256_add(
    _input: &[u8],
) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()

    /*use tbn::{AffineG1, Fq, G1, Group};
    debug_print!("bn256Add");

    let return_buf = |buf: [u8; 64]| {
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf.to_vec()))
    };

    let mut buf = [0_u8; 64];

    let a = if let Some(a) = self.get_g1(input) {
        a
    } else {
        debug_print!("Invalid point x : G1");
        return return_buf(buf)
    };

    let (_, input) = input.split_at(64);

    if input.len() < 64 {
        if a.is_zero() {
            return return_buf(buf)
        }

        a.x().to_big_endian(&mut buf[0..32]);
        a.y().to_big_endian(&mut buf[32..64]);

        return return_buf(buf)
    }

    let b = if let Some(b) = self.get_g1(input) {
        b
    } else {
        debug_print!("Invalid point b : G1");
        return return_buf(buf)
    };

    if let Some(sum) = AffineG1::from_jacobian(a + b) {
        // point not at infinity
        if sum.x().to_big_endian(&mut buf[0..32]).is_err() {
            return return_buf(buf)
        }
        if sum.y().to_big_endian(&mut buf[32..64]).is_err() {
            return return_buf(buf)
        }
    } else {
        debug_print!("Invalid point (a + b)");
    }

    return_buf(buf)*/
}

/// Call inner `bn256ScalarMul`
#[must_use]
#[allow(unused)]
pub fn bn256_scalar_mul(
    _input: &[u8],
) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()

    /*use tbn::{AffineG1, Fr, Group};
    debug_print!("bn256ScalarMul");

    let return_buf = |buf: [u8; 64]| {
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf.to_vec()))
    };

    let mut buf = [0_u8; 64];

    let a = if let Some(a) = self.get_g1(input) {
        a
    } else {
        debug_print!("Invalid point x : G1");
        return return_buf(buf)
    };

    let (_, input) = input.split_at(64);

    if input.len() < 32 {
        if a.is_zero() {
            return return_buf(buf)
        }
        if a.x().to_big_endian(&mut buf[0..32]).is_err() {
            return return_buf(buf)
        }
        if a.y().to_big_endian(&mut buf[32..64]).is_err() {
            return return_buf(buf)
        }
        return return_buf(buf)
    }

    let (s_slice, _input) = input.split_at(32);

    let s = if let Ok(s) = Fr::from_slice(s_slice) {
        s
    } else {
        return return_buf(buf)
    };

    if let Some(sum) = AffineG1::from_jacobian(a * s) {
        // point not at infinity
        if sum.x().to_big_endian(&mut buf[0..32]).is_err() {
            return return_buf(buf)
        }
        if sum.y().to_big_endian(&mut buf[32..64]).is_err() {
            return return_buf(buf)
        }
    }

    return_buf(buf)*/
}

/// Call inner `bn256Pairing`
#[must_use]
#[allow(unused)]
pub fn bn256_pairing(
    _input: &[u8],
) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()
    
    /*
    use tbn::{AffineG1, AffineG2, Fq, Fq2, pairing_batch, G1, G2, Gt, Group, GroupError};
    debug_print!("bn256Pairing");

    let return_err = || {
        Capture::Exit((ExitReason::Error(evm::ExitError::InvalidRange), Vec::new()))
    };
    let return_val = |result: bool| {
        let mut buf = [0_u8; 32];
        if result {
            U256::one().to_big_endian(&mut buf);
            return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf.to_vec()))
        }
        Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), buf.to_vec()))
    };

    if input.len() % 192 > 0 {
        return return_err()
    }
    if input.is_empty() {
        return return_val(true)
    }

    let mut vals = Vec::new();
    for chunk in input.chunks(192) {
        let a = if let Some(a) = self.get_g1(chunk) {
            a
        } else {
            debug_print!("Invalid point a : G1");
            return return_err()
        };

        let (_, chunk) = chunk.split_at(64);

        let (ax_slice, chunk) = chunk.split_at(32);
        let (ay_slice, chunk) = chunk.split_at(32);
        let (bx_slice, by_slice) = chunk.split_at(32);

        let fq_ax = if let Ok(fq_ax) = Fq::from_slice(ax_slice) {
            fq_ax
        } else {
            debug_print!("Invalid Fq point");
            return return_err()
        };
        let fq_ay = if let Ok(fq_ay) = Fq::from_slice(ay_slice) {
            fq_ay
        } else {
            debug_print!("Invalid Fq point");
            return return_err()
        };
        let fq_bx = if let Ok(fq_bx) = Fq::from_slice(bx_slice) {
            fq_bx
        } else {
            debug_print!("Invalid Fq point");
            return return_err()
        };
        let fq_by = if let Ok(fq_by) = Fq::from_slice(by_slice) {
            fq_by
        } else {
            debug_print!("Invalid Fq point");
            return return_err()
        };

        let b_a = Fq2::new(fq_ay, fq_ax);
        let b_b = Fq2::new(fq_by, fq_bx);

        let b : G2 = if b_a.is_zero() && b_b.is_zero() {
            G2::zero()
        } else {
            match AffineG2::new(b_a, b_b) {
                Ok(b) => b.into(),
                Err(GroupError::NotOnCurve) => {
                    debug_print!("Invalid G2 point: NotOnCurve");
                    return return_err()
                },
                Err(GroupError::NotInSubgroup) => {
                    debug_print!("Invalid G2 point: NotInSubgroup");
                    return return_err()
                }
            }
        };

        vals.push((a, b));
    }

    if pairing_batch(&vals) == Gt::one() {
        return return_val(true)
    }

    return_val(false)
    */
}