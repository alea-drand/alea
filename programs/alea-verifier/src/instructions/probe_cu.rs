// TEMPORARY — Phase 1.1.D probe. Remove in Phase 2.
use anchor_lang::prelude::*;
use anchor_lang::solana_program::log::sol_log_compute_units;
use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, Field};

#[derive(Accounts)]
pub struct ProbeCu<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
}

pub fn handler(_ctx: Context<ProbeCu>) -> Result<()> {
    let x = Fq::from(42u64);

    // (p-1)/2 as [u64; 4] little-endian limbs
    let exp: [u64; 4] = [
        0x9E10460B6C3E7EA3,
        0xCBC0B548B438E546,
        0xDC2822DB40C0AC2E,
        0x183227397098D014,
    ];

    msg!("=== Fq::pow benchmark ===");
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
