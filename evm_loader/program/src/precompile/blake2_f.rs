use std::convert::Infallible;

use evm::{Capture, ExitReason};


#[must_use]
#[allow(clippy::too_many_lines)]
pub fn blake2_f(
    input: &[u8]
) -> Capture<(ExitReason, Vec<u8>), Infallible> {
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
            v[14] = !v[14]; // Invert all bits if the last-block-flag is set.
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
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new()));
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

    let f = if input[0] == 1 {
        true
    } else if input[0] == 0 {
        false
    } else {
        // return Err(ExitError::Other("incorrect final block indicator flag".into()))
        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), Vec::new()));
    };

    compress(&mut h, m, [t_0, t_1], f, rounds as usize);

    let mut output_buf = [0_u8; 64];
    for (i, state_word) in h.iter().enumerate() {
        output_buf[i * 8..(i + 1) * 8].copy_from_slice(&state_word.to_le_bytes());
    }

    debug_print!("{}", &hex::encode(output_buf));

    Capture::Exit((
        ExitReason::Succeed(evm::ExitSucceed::Returned),
        output_buf.to_vec(),
    ))
}
