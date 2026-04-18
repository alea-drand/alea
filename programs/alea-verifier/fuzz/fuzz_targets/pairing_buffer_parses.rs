#![no_main]

// Fuzz target: pairing buffer EIP-197 layout robustness
//
// Stage 8 addition (Phase 2.5 Wave G). Goal: confirm that `on_curve_g1`
// never panics on any 64-byte input, and that negate_g1 never panics
// on any 64-byte input that passed on_curve_g1. These two primitives
// are the ONLY pieces of Alea's pairing pipeline that see attacker
// input before the alt_bn128_pairing syscall. A panic here would be
// a BPF tx abort with opaque error.
//
// Also spot-checks the `bytes_to_bigint` helper (used inside
// on_curve_g1) for any input that triggers overflow in the limb-
// extraction loop.
//
// Invariants:
//   1. on_curve_g1 NEVER panics on any 64-byte input
//   2. negate_g1 NEVER panics on any 64-byte input (it's pub and used
//      internally; defense-in-depth on its precondition)
//   3. If on_curve_g1 returns true, negate_g1 output is also on-curve
//      (negation preserves curve membership)

use alea_verifier::crypto::pairing::{negate_g1, on_curve_g1};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: [u8; 64]| {
    // INVARIANT 1: on_curve_g1 must not panic on any input.
    let is_on_curve = on_curve_g1(&bytes);

    // INVARIANT 2: negate_g1 is currently `pub` and takes `&[u8; 64]`.
    // It MUST not panic regardless of input (defensive contract even
    // though internal callers always pre-validate).
    let negated = negate_g1(&bytes);

    // INVARIANT 3: if input is on curve, negated must ALSO be on curve
    // (negation preserves curve membership: (x, y) on curve ⇔ (x, -y)
    // on curve since y² = x³ + 3 implies (-y)² = x³ + 3).
    if is_on_curve {
        assert!(
            on_curve_g1(&negated),
            "negate_g1 produced off-curve point from on-curve input: \
             input={}, negated={}",
            hex::encode(&bytes[..]),
            hex::encode(&negated[..])
        );
    }
});
