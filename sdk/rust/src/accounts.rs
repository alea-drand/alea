//! Account types and helpers for Alea CPI consumers.

use alea_verifier::program::AleaVerifier as AleaVerifierProgram;
use anchor_lang::prelude::*;

pub use alea_verifier::state::Config;

/// Convenience Accounts fragment for a CPI verify call.
///
/// Embed this in your consumer's Accounts struct (or copy the two fields
/// individually) to get Anchor's automatic discriminator + owner checks on
/// `config` and the MANDATORY `seeds::program` constraint (ADR 0034).
///
/// # Why `seeds::program` is MANDATORY
///
/// Without it, an attacker can substitute a fake config PDA owned by a
/// different program. Anchor does not enforce the program-ownership check
/// on PDA seeds by default — `seeds::program` is the explicit opt-in that
/// re-derives the PDA using Alea's program ID as the signer. See ADR 0034
/// and `build-spec/sdk/rust-cpi.md §"Security: Mandatory Constraints"`.
///
/// # Usage
///
/// ```rust,ignore
/// use alea_sdk::{AleaVerify, AleaVerifier};
///
/// #[derive(Accounts)]
/// pub struct SettleMatch<'info> {
///     // … your program's accounts …
///     pub alea_program: Program<'info, AleaVerifier>,
///     #[account(
///         seeds = [b"config"],
///         bump,
///         seeds::program = alea_program.key(),   // ← MANDATORY
///     )]
///     pub alea_config: Account<'info, alea_sdk::Config>,
///     pub payer: Signer<'info>,
///     pub clock: Sysvar<'info, Clock>,
/// }
/// ```
#[derive(Accounts)]
pub struct AleaVerify<'info> {
    /// Alea program (executable). Constrained to `PROGRAM_ID` by Anchor's
    /// `Program` type.
    pub alea_program: Program<'info, AleaVerifierProgram>,

    /// Alea Config PDA (read-only). MUST include `seeds::program` to guard
    /// against fake-config substitution attacks (ADR 0034).
    #[account(
        seeds = [b"config"],
        bump = config.bump,
        seeds::program = alea_program.key(),
    )]
    pub config: Account<'info, Config>,
}
