//! # alea-sdk
//!
//! CPI crate for Alea — the first production drand BN254 BLS verifier on
//! Solana. Any Anchor program can receive verified on-chain randomness with
//! a single CPI call.
//!
//! ## Quick Start
//!
//! Add the mandatory constraints to your Accounts struct and call `cpi::verify`:
//!
//! ```rust,ignore
//! use alea_sdk::{self, AleaVerifier};
//! use anchor_lang::solana_program::sysvar::clock::Clock;
//!
//! const MAX_BEACON_AGE_SECONDS: u64 = 30;
//!
//! #[derive(Accounts)]
//! pub struct SettleMatch<'info> {
//!     pub alea_program: Program<'info, AleaVerifier>,
//!     #[account(
//!         seeds = [b"config"],
//!         bump,
//!         seeds::program = alea_program.key(),   // ← MANDATORY (ADR 0034)
//!     )]
//!     pub alea_config: Account<'info, alea_sdk::Config>,
//!     pub payer: Signer<'info>,
//!     pub clock: Sysvar<'info, Clock>,
//! }
//!
//! pub fn settle_match(ctx: Context<SettleMatch>, round: u64, sig: [u8; 64]) -> Result<()> {
//!     // MANDATORY: reject stale beacons before CPI
//!     require!(
//!         alea_sdk::is_round_recent(round, &ctx.accounts.alea_config, &ctx.accounts.clock, MAX_BEACON_AGE_SECONDS),
//!         YourError::StaleBeacon,
//!     );
//!     // One-line CPI. Returns VerifiedRandomness (must_use wrapper).
//!     let randomness = alea_sdk::cpi::verify(
//!         ctx.accounts.alea_program.to_account_info(),
//!         ctx.accounts.alea_config.to_account_info(),
//!         ctx.accounts.payer.to_account_info(),
//!         round, sig,
//!     )?.into_inner();
//!     // Read IMMEDIATELY — Solana return data is overwritten by any subsequent CPI
//!     let random_value = u64::from_le_bytes(randomness[0..8].try_into().unwrap());
//!     // … use randomness …
//!     Ok(())
//! }
//! ```
//!
//! ## Security: Mandatory Constraints
//!
//! Two constraints are MANDATORY for ANY consumer (omitting either ships an
//! exploitable program):
//!
//! 1. **`seeds::program = alea_program.key()`** on the `alea_config` account.
//!    Without this, an attacker can substitute a fake Config PDA owned by a
//!    different program and feed attacker-controlled public keys to the pairing
//!    check. This is total compromise for any randomness consumer. (ADR 0034)
//!
//! 2. **`is_round_recent()` before trusting randomness.** Without recency
//!    enforcement, an attacker can replay an old drand round whose randomness
//!    they already know to bias resolution.
//!
//! ## CPI Return Data Ordering Warning
//!
//! Solana's return data is single-slot — each CPI call overwrites the
//! previous value. Read `cpi::verify`'s result into a local variable
//! IMMEDIATELY, before any other CPI calls (token transfers, etc.).
//!
//! ## Compute Budget
//!
//! Every transaction calling Alea MUST include a compute budget instruction
//! of at least 900,000 CU (Solana default is 200K; Alea needs up to 454K +
//! consumer headroom). The TypeScript SDK injects this automatically.
//!
//! ## Program IDs
//!
//! | Cluster | Program ID |
//! |---------|-----------|
//! | Devnet  | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` |
//! | Mainnet | Pending Phase 5 (same vanity ID — cluster binding is identical) |
//!
//! Devnet-verified; mainnet deployment pending Phase 5. Cluster binding
//! identical (vanity ID usable on both), mainnet traffic begins Phase 5.
//!
//! ## Maturity
//!
//! See [CAVEATS.md](https://github.com/alea-drand/alea/blob/main/sdk/rust/CAVEATS.md)
//! for maturity disclosures before integrating.

#![deny(unsafe_code)]
// Suppress Anchor 0.30.1's harmless `anchor-debug` cfg warning (emitted by
// the #[derive(Accounts)] macro). Same suppression that
// programs/alea-verifier/src/lib.rs carries.
#![allow(unexpected_cfgs)]

pub mod accounts;
pub mod cpi;
pub mod errors;

pub use accounts::Config;
pub use alea_verifier::errors::AleaError;
pub use alea_verifier::program::AleaVerifier;
pub use cpi::VerifiedRandomness;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::clock::Clock;

/// Canonical Alea program ID. Vanity, frozen for the lifetime of the
/// mainnet deployment per ADR 0028. Same ID used across localnet / devnet
/// / mainnet by design — consumer SDKs do not need to branch per cluster.
///
/// Devnet-verified; mainnet deployment pending Phase 5. Cluster binding
/// identical (vanity ID usable on both), mainnet traffic begins Phase 5.
///
/// This re-exports the verifier crate's `declare_id!`-generated `ID`
/// constant, which guarantees the SDK's PROGRAM_ID can never drift from
/// the program's on-chain identity at compile time.
pub const PROGRAM_ID: Pubkey = alea_verifier::ID;

/// Derive the Alea `Config` PDA for a given program ID.
///
/// Seeds are `[b"config"]`. The canonical bump is stored in `Config::bump`
/// at initialization; consumer programs using `bump = config.bump` skip
/// re-derivation (~10K CU saving per ADR 0028 §"PDA derivation").
pub fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"config"], program_id)
}

/// Check that a drand round is recent relative to the current on-chain
/// clock. Returns `true` if the round's emission timestamp is within
/// `max_age_seconds` of the current slot's `unix_timestamp`.
///
/// # Why this exists
///
/// Alea's `verify` instruction accepts ANY round — including very old
/// ones. Any consumer program where randomness resolves a high-stakes
/// outcome (games, lotteries, prediction markets) MUST enforce recency
/// before trusting the verified randomness, otherwise an attacker can
/// replay a known-randomness beacon from months ago. This is a
/// **consumer-layer responsibility** — Alea itself is stateless by
/// design and cannot enforce recency without adding accounts / CPI cost.
///
/// # Parameters
///
/// - `round`: the drand round number being verified
/// - `config`: the Alea `Config` PDA (read for `genesis_time` and `period`)
/// - `clock`: the Solana `Clock` sysvar (for `unix_timestamp`)
/// - `max_age_seconds`: rejection threshold. `30` is a reasonable default
///   for most consumers; tighten to `3` (one drand round) for adversarial
///   contexts like MEV-resistant lotteries.
///
/// # Overflow safety
///
/// `saturating_sub` + `saturating_mul` on all arithmetic. A malformed
/// `round == u64::MAX` with realistic genesis/period values would
/// otherwise overflow in `(round - 1) * period`. Saturation is preferred
/// over wrapping because "stale" is the safe rejection outcome.
pub fn is_round_recent(round: u64, config: &Config, clock: &Clock, max_age_seconds: u64) -> bool {
    let round_timestamp = config
        .genesis_time
        .saturating_add(round.saturating_sub(1).saturating_mul(config.period));
    // Phase 4.5 T2-01: clamp negative i64 unix_timestamp to 0 before the u64
    // cast. Solana's live clock is always positive; this guard handles
    // localnet clock quirks, misconfigured validators, and hypothetical
    // future runtime bugs. Without the clamp, a negative i64 wraps to a huge
    // u64, making all recency checks return stale (false) until clock
    // normalizes — availability impact for any consumer that calls verify.
    let current_timestamp = clock.unix_timestamp.max(0) as u64;
    current_timestamp.saturating_sub(round_timestamp) <= max_age_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_id_matches_expected_vanity() {
        assert_eq!(
            PROGRAM_ID.to_string(),
            "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U",
            "PROGRAM_ID must match vanity declared in programs/alea-verifier/src/lib.rs \
             declare_id! — any drift is a CPI-stability violation per ADR 0028"
        );
    }

    #[test]
    fn config_pda_is_deterministic() {
        let (pda_a, bump_a) = config_pda(&PROGRAM_ID);
        let (pda_b, bump_b) = config_pda(&PROGRAM_ID);
        assert_eq!(pda_a, pda_b);
        assert_eq!(bump_a, bump_b);
    }
}
