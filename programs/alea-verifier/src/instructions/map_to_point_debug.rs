//! T1.04 — BPF-vs-native `map_to_point` parity debug instruction.
//!
//! Takes 32 bytes of field-element input (big-endian representation
//! of `u ∈ Fq`) and returns 64 bytes (`x || y`, both BE) = the output
//! of `map_to_point(u)`. Fires `AleaError::NoSquareRoot` (6004) if all
//! three SVDW candidates fail (matches production `hash_to_g1`'s
//! propagation path).
//!
//! SECURITY POSTURE (see `instructions/mod.rs` for full rationale):
//! stateless pure function, no authority check, no state mutation,
//! computation freely available off-chain via gnark-crypto. This
//! instruction is INTENTIONALLY always-present in the shipped binary
//! — not feature-gated — because Anchor 0.30.1's `#[program]` macro
//! emits client-account bindings that don't respect `cfg` on a single
//! inner function, and carrying the instruction as always-on with zero
//! attack surface is cleaner than maintaining two parallel `#[program]`
//! modules. The `mod.rs` SECURITY POSTURE block documents the full
//! risk analysis.
//!
//! Test harness (`tests/map-to-point-diff.ts`) calls this with the
//! gnark-crypto-verified `u0_hex`/`u1_hex` values from the existing
//! round-1 / round-9337227 fixtures and asserts byte-equality with
//! the expected `Q0_x_hex`/`Q0_y_hex`/`Q1_x_hex`/`Q1_y_hex` outputs.
//! 8 direct BPF-vs-gnark byte-equality assertions — closes T1.04
//! (AUDIT-REPORT-R5.md §T1.04).

use anchor_lang::prelude::*;

use crate::crypto::svdw::{fq_from_be_bytes, map_to_point};
use crate::errors::AleaError;

/// Accounts struct for the debug instruction. Stateless — only needs a
/// signer for tx fees; no PDAs, no config, no side effects.
#[derive(Accounts)]
pub struct MapToPointDebug<'info> {
    /// Signer pays fees. No authorization required — debug instruction
    /// is compile-time gated by `diff-test` feature, not run-time.
    pub payer: Signer<'info>,
}

/// Invoke `map_to_point(u)` on BPF and return the 64-byte result.
///
/// Input: 32 bytes = `u.into_bigint()` big-endian.
/// Output: 64 bytes = `x || y` big-endian.
/// Failure: `AleaError::NoSquareRoot` (6004) if SVDW theorem violation
/// (all three x candidates fail try_sqrt_curve — should not occur for
/// honest inputs per the theorem; surfaces a syscall oracle regression).
pub fn map_to_point_debug_handler(
    _ctx: Context<MapToPointDebug>,
    u_bytes: [u8; 32],
) -> Result<[u8; 64]> {
    let u = fq_from_be_bytes(&u_bytes);
    let result = map_to_point(&u).ok_or(AleaError::NoSquareRoot)?;
    Ok(result)
}
