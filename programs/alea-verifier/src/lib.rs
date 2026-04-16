#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::alt_bn128::compression::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;

declare_id!("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U");

pub mod crypto;

#[derive(Accounts)]
pub struct ProbeSyscall<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

#[program]
pub mod alea_verifier {
    use super::*;

    pub fn probe_syscall(_ctx: Context<ProbeSyscall>) -> Result<()> {
        // Test 1: G1 decompression with a known valid x-coordinate
        // x = 1 → gx = 1³+3 = 4, sqrt(4) = 2 → valid point (1, 2)
        let mut x_bytes = [0u8; 32];
        x_bytes[31] = 1; // x = 1, big-endian

        msg!("=== G1 decompress: x=1 (valid, sqrt(4)=2) ===");
        sol_log_compute_units();
        let result = alt_bn128_g1_decompress(&x_bytes);
        sol_log_compute_units();
        match &result {
            Ok(point) => {
                msg!("OK! y[31]={}", point[63]);
                // y should be 2 (or p-2)
            }
            Err(e) => msg!("ERR: {:?}", e),
        }

        // Test 2: G1 decompression with x=0 → gx = 0³+3 = 3
        // Is 3 a QR mod p? From research: NO (p ≡ 7 mod 12 → 3 is non-residue)
        let x_zero = [0u8; 32];
        msg!("=== G1 decompress: x=0 (3 is non-QR, should fail) ===");
        sol_log_compute_units();
        let result2 = alt_bn128_g1_decompress(&x_zero);
        sol_log_compute_units();
        match &result2 {
            Ok(_) => msg!("OK (unexpected — 0 is identity?)"),
            Err(e) => msg!("ERR: {:?}", e),
        }

        // Test 3: x=4 → gx = 64+3 = 67. 67 is NON-QR mod p (verified via Python)
        let mut x4_bytes = [0u8; 32];
        x4_bytes[31] = 4;
        msg!("=== G1 decompress: x=4 (gx=67, NON-QR — should fail) ===");
        sol_log_compute_units();
        let result3 = alt_bn128_g1_decompress(&x4_bytes);
        sol_log_compute_units();
        match &result3 {
            Ok(_) => msg!("OK (UNEXPECTED — 67 should be non-QR!)"),
            Err(e) => msg!("ERR: {:?} (CORRECT — 67 is non-QR)", e),
        }

        Ok(())
    }
}
