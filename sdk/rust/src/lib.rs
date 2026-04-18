//! # alea-sdk — minimal Phase 3 scaffold
//!
//! Full CPI surface (verify wrapper, AleaVerify accounts helper) ships in
//! Phase 4. This crate presently exposes only what Phase 3 live-Clock test
//! and downstream consumers actually need:
//!
//! - `is_round_recent` — consumer-layer stale-beacon guard (mandatory per
//!   `build-spec/sdk/rust-cpi.md §"Security: Mandatory Constraints"`)
//! - `config_pda` — deterministic PDA derivation for the Alea `Config`
//! - `PROGRAM_ID` — canonical vanity program ID (frozen per ADR 0028)
//! - Re-exports of `Config` and `AleaError` from the verifier crate
//!
//! See `build-spec/sdk/rust-cpi.md` for the full v1 API surface (Phase 4).

#![deny(unsafe_code)]

pub use alea_verifier::errors::AleaError;
pub use alea_verifier::state::Config;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::clock::Clock;

/// Canonical Alea program ID. Vanity, frozen for the lifetime of the
/// mainnet deployment per ADR 0028. Same ID used across localnet / devnet
/// / mainnet by design — consumer SDKs do not need to branch per cluster.
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
pub fn is_round_recent(
    round: u64,
    config: &Config,
    clock: &Clock,
    max_age_seconds: u64,
) -> bool {
    let round_timestamp = config
        .genesis_time
        .saturating_add(round.saturating_sub(1).saturating_mul(config.period));
    let current_timestamp = clock.unix_timestamp as u64;
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
