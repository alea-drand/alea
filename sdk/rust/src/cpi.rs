//! CPI wrapper for Alea's `verify` instruction.
//!
//! Consumer programs should call [`verify`] instead of invoking the raw
//! Anchor-generated `alea_verifier::cpi::verify` directly — this helper
//! captures the return data in the same expression, preventing the ordering
//! footgun described below.
//!
//! # CRITICAL: Return data ordering
//!
//! Solana's return data is single-slot — each CPI call that sets return data
//! overwrites the previous value. Consumer programs MUST read the randomness
//! into a local variable immediately after this call, BEFORE making any other
//! CPI calls (token transfers, system program, etc.):
//!
//! ```rust,ignore
//! // CORRECT — capture first, then downstream CPIs are safe
//! let randomness = alea_sdk::cpi::verify(/* args */)?;
//! token::transfer(transfer_ctx, amount)?;
//!
//! // WRONG — token::transfer overwrites Alea's return data
//! token::transfer(transfer_ctx, amount)?;
//! let randomness = alea_sdk::cpi::verify(/* args */)?; // stale/empty
//! ```

use anchor_lang::prelude::*;

/// Verify a drand beacon via CPI to Alea and receive 32 bytes of randomness.
///
/// Pattern A (auto-deserialize) per ADR 0030. Anchor 0.30.x's generated
/// `alea_verifier::cpi::verify(...)` returns `Result<Return<[u8; 32]>>`;
/// `.get()` unwraps to the deserialized `[u8; 32]` directly. No
/// `get_return_data()` call is required.
///
/// # Arguments
/// * `alea_program` — the Alea program (must be `ALEAydzHd…` per
///   [`crate::PROGRAM_ID`])
/// * `config` — the Alea Config PDA (must be checked in the consumer's
///   Accounts struct with `seeds::program = alea_program.key()` — see
///   the `lib.rs` doc example and ADR 0034)
/// * `payer` — signer, passed through to Alea's Verify accounts struct
/// * `round` — drand round number
/// * `signature` — 64-byte G1 point (uncompressed, x || y big-endian)
///
/// # Errors
/// Returns Alea's on-chain error codes (6000-6009) for signature,
/// chain-hash, or field-arithmetic failures. See [`crate::AleaError`].
#[must_use = "Alea's return data is single-slot — capture randomness immediately; any later CPI overwrites it"]
pub fn verify<'info>(
    alea_program: AccountInfo<'info>,
    config: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    round: u64,
    signature: [u8; 64],
) -> Result<[u8; 32]> {
    // Defense-in-depth (Phase 4.5 T1-08): the mandatory `seeds::program`
    // guard lives in the consumer's #[derive(Accounts)], which is the
    // strong defense for Anchor-idiomatic callers. Non-Anchor callers
    // (raw process_instruction handlers, governance relays, CPI
    // forwarders) bypass that check. A runtime owner assertion here
    // closes the gap at ~200 CU cost (0.04% of the 900K budget).
    require_keys_eq!(*config.owner, crate::PROGRAM_ID, crate::AleaError::WrongPubkey);
    let accounts = alea_verifier::cpi::accounts::Verify {
        config: config.clone(),
        payer: payer.clone(),
    };
    let cpi_ctx = CpiContext::new(alea_program, accounts);
    let randomness = alea_verifier::cpi::verify(cpi_ctx, round, signature)?.get();
    Ok(randomness)
}
