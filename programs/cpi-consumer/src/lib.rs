//! Test-only fixture: demonstrates CPI to alea_verifier::verify.
//!
//! Localnet only. Never deploys to devnet or mainnet. Exists purely to
//! exercise the P0 test #12 acceptance criterion: a consumer program
//! can CPI to Alea and receive the 32-byte randomness via Anchor return
//! data (ADR 0030 — Pattern A empirical validation).

// Suppress Anchor 0.30.1's harmless `anchor-debug` cfg warning (same as
// programs/alea-verifier/src/lib.rs).
#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

use alea_verifier::{
    cpi::{accounts::Verify as AleaVerifyAccounts, verify as alea_verify},
    program::AleaVerifier,
    state::Config as AleaConfig,
};

declare_id!("CjG5jdi4unFM2QJ1Z46kB58fHpGbtiEj2m8PmEyPSKvj");

#[program]
pub mod cpi_consumer {
    use super::*;

    /// Call alea_verifier::verify via CPI and return the randomness.
    pub fn consume_randomness(
        ctx: Context<ConsumeRandomness>,
        round: u64,
        signature: [u8; 64],
    ) -> Result<[u8; 32]> {
        let cpi_accounts = AleaVerifyAccounts {
            config: ctx.accounts.alea_config.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
        };
        let cpi_program = ctx.accounts.alea_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        let randomness_return = alea_verify(cpi_ctx, round, signature)?;
        let randomness: [u8; 32] = randomness_return.get();

        msg!("cpi-consumer received randomness: {:?}", randomness);
        Ok(randomness)
    }
}

#[derive(Accounts)]
pub struct ConsumeRandomness<'info> {
    /// Alea program. Must match the deployed alea_verifier program ID
    /// (enforced by Anchor's `Program` type using `alea_verifier::ID`).
    pub alea_program: Program<'info, AleaVerifier>,

    /// Alea config PDA.
    ///
    /// T2.B — typed as `Account<'info, AleaConfig>` so Anchor performs
    /// discriminator + owner validation on the consumer side (belt-and-
    /// suspenders over `seeds::program`). Previously `UncheckedAccount`,
    /// which relied 100% on `seeds::program` — if a consumer copied this
    /// fixture and omitted `seeds::program`, they had zero protection.
    /// Now the type system catches substitution attempts at deserialization
    /// even if `seeds::program` is stripped.
    ///
    /// `seeds::program = alea_program.key()` is still the primary defense
    /// per ADR 0034 (mandatory for all CPI consumers). Both layers together
    /// = defense-in-depth. Sources: P07-T2-02, P08-T3-01.
    #[account(
        seeds = [b"config"],
        bump,
        seeds::program = alea_program.key(),
    )]
    pub alea_config: Account<'info, AleaConfig>,

    /// End user / payer for the verify call. Signed by the tx submitter.
    pub payer: Signer<'info>,
}
