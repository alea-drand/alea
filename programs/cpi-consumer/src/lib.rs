//! Test-only fixture: demonstrates CPI to alea_verifier::verify.
//!
//! Localnet only. Never deploys to devnet or mainnet. Exists purely to
//! exercise the P0 test #12 acceptance criterion: a consumer program
//! can CPI to Alea and receive the 32-byte randomness via Anchor return
//! data (ADR 0030 — Pattern A empirical validation).

use anchor_lang::prelude::*;

use alea_verifier::{
    cpi::{accounts::Verify as AleaVerifyAccounts, verify as alea_verify},
    program::AleaVerifier,
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

    /// Alea config PDA. `seeds::program` constraint (ADR 0034) ensures
    /// the PDA is derived from the Alea program ID — not from cpi-consumer's
    /// own ID or some other program. This is the consumer-side guard against
    /// substitution attacks.
    /// CHECK: deserialized by alea_verifier during CPI.
    #[account(
        seeds = [b"config"],
        bump,
        seeds::program = alea_program.key(),
    )]
    pub alea_config: UncheckedAccount<'info>,

    /// End user / payer for the verify call. Signed by the tx submitter.
    pub payer: Signer<'info>,
}
