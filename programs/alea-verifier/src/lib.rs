#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::alt_bn128::compression::prelude::*;
use anchor_lang::solana_program::big_mod_exp::big_mod_exp;
use anchor_lang::solana_program::log::sol_log_compute_units;
use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, Field, PrimeField};

declare_id!("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U");

pub mod crypto;

// BN254 base field prime (big-endian)
const P_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
    0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d,
    0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

// (p+1)/4 exponent for sqrt (big-endian)
const SQRT_EXP_BE: [u8; 32] = [
    0x0c, 0x19, 0x13, 0x9c, 0xb8, 0x4c, 0x68, 0x0a,
    0x6e, 0x14, 0x11, 0x6d, 0xa0, 0x60, 0x56, 0x17,
    0x65, 0xe0, 0x5a, 0xa4, 0x5a, 0x1c, 0x72, 0xa3,
    0x4f, 0x08, 0x23, 0x05, 0xb6, 0x1f, 0x3f, 0x52,
];

#[derive(Accounts)]
pub struct ProbeSyscall<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

#[program]
pub mod alea_verifier {
    use super::*;

    pub fn probe_syscall(_ctx: Context<ProbeSyscall>) -> Result<()> {
        // x = 1, gx = 1³+3 = 4, sqrt(4) = 2
        let mut x_bytes = [0u8; 32];
        x_bytes[31] = 1;

        // ========== SOLUTION A: G1 Decompression ==========
        msg!("=== A: G1 decompress (valid x=1) ===");
        sol_log_compute_units();
        let result_a = alt_bn128_g1_decompress(&x_bytes);
        sol_log_compute_units();
        match &result_a {
            Ok(pt) => msg!("A OK: y_last_byte={}", pt[63]),
            Err(e) => msg!("A ERR: {:?}", e),
        }

        // A with non-QR (x=4, gx=67)
        let mut x4_bytes = [0u8; 32];
        x4_bytes[31] = 4;
        msg!("=== A: G1 decompress (non-QR x=4) ===");
        sol_log_compute_units();
        let result_a2 = alt_bn128_g1_decompress(&x4_bytes);
        sol_log_compute_units();
        match &result_a2 {
            Ok(_) => msg!("A OK (unexpected)"),
            Err(_) => msg!("A ERR (correct, non-QR)"),
        }

        // ========== SOLUTION B: big_mod_exp ==========
        // Compute x^((p+1)/4) mod p = sqrt(x) in the field
        // base = 4 (we know sqrt(4)=2)
        let mut base_bytes = [0u8; 32];
        base_bytes[31] = 4;

        msg!("=== B: big_mod_exp sqrt(4) ===");
        sol_log_compute_units();
        let result_b = big_mod_exp(&base_bytes, &SQRT_EXP_BE, &P_BE);
        sol_log_compute_units();
        msg!("B result last byte: {}", result_b[31]);
        // Should be 2

        // big_mod_exp with larger input (x=42³+3)
        let val_42 = Fq::from(42u64);
        let gx_42 = val_42 * val_42 * val_42 + Fq::from(3u64);
        let gx_42_bigint = gx_42.into_bigint();
        let mut gx_42_be = [0u8; 32];
        for (i, limb) in gx_42_bigint.0.iter().enumerate() {
            let be = limb.to_be_bytes();
            for j in 0..8 {
                gx_42_be[24 - i * 8 + j] = be[j];
            }
        }

        msg!("=== B: big_mod_exp sqrt(42^3+3) ===");
        sol_log_compute_units();
        let result_b2 = big_mod_exp(&gx_42_be, &SQRT_EXP_BE, &P_BE);
        sol_log_compute_units();
        msg!("B result len: {}", result_b2.len());

        // ========== SOLUTION C: Addition Chain (already measured) ==========
        msg!("=== C: Addition chain sqrt(4) via optimized_exp ===");
        sol_log_compute_units();
        let fq4 = Fq::from(4u64);
        let result_c = crypto::optimized_exp::sqrt_and_check(&fq4);
        sol_log_compute_units();
        match result_c {
            Some(v) => msg!("C OK: is_two={}", v == Fq::from(2u64) || v == -(Fq::from(2u64))),
            None => msg!("C: no sqrt"),
        }

        msg!("=== ALL DONE ===");
        Ok(())
    }
}
