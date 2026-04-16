#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;
use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, Field};

declare_id!("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U");

pub mod crypto;

#[derive(Accounts)]
pub struct ProbeCu<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ProbeOptimized<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

const SQRT_EXP: [u64; 4] = [
    0x4F082305B61F3F52,
    0x65E05AA45A1C72A3,
    0x6E14116DA0605617,
    0x0C19139CB84C680A,
];

#[program]
pub mod alea_verifier {
    use super::*;

    pub fn probe_cu(_ctx: Context<ProbeCu>) -> Result<()> {
        let x = Fq::from(42u64);
        let exp: [u64; 4] = [
            0x9E10460B6C3E7EA3,
            0xCBC0B548B438E546,
            0xDC2822DB40C0AC2E,
            0x183227397098D014,
        ];

        msg!("=== Fq::pow (p-1)/2 benchmark ===");
        sol_log_compute_units();
        let pow_result = x.pow(exp);
        sol_log_compute_units();
        msg!("pow done, nonzero={}", pow_result != Fq::ZERO);

        msg!("=== Fq::sqrt benchmark ===");
        sol_log_compute_units();
        let sqrt_result = x.sqrt();
        sol_log_compute_units();
        msg!("sqrt done, is_some={}", sqrt_result.is_some());

        msg!("=== Fq::inverse benchmark ===");
        sol_log_compute_units();
        let inv_result = x.inverse();
        sol_log_compute_units();
        msg!("inverse done, is_some={}", inv_result.is_some());

        Ok(())
    }

    pub fn probe_optimized(_ctx: Context<ProbeOptimized>) -> Result<()> {
        // Simulate a SINGLE map_to_point with sqrt-and-check optimization
        let u = Fq::from(42u64);

        msg!("=== sqrt_and_check #1 (simulating first candidate) ===");
        sol_log_compute_units();
        let gx1 = u * u * u + Fq::from(3u64);
        let s1 = gx1.pow(SQRT_EXP);
        let is_qr1 = s1 * s1 == gx1;
        sol_log_compute_units();
        msg!("check1 done, is_qr={}", is_qr1);

        msg!("=== sqrt_and_check #2 (simulating second candidate) ===");
        sol_log_compute_units();
        let gx2 = (u + Fq::from(1u64)).pow([3u64, 0, 0, 0]) + Fq::from(3u64);
        let s2 = gx2.pow(SQRT_EXP);
        let is_qr2 = s2 * s2 == gx2;
        sol_log_compute_units();
        msg!("check2 done, is_qr={}", is_qr2);

        msg!("=== total for simulated map_to_point (2 candidates) ===");

        Ok(())
    }
}
