#![no_main]

// Fuzz target: hash_to_g1 (isolate hash-to-curve from pairing)
//
// Goal: find any input (round number OR arbitrary message bytes) that causes
// hash_to_g1 / hash_round_to_g1 to panic or produce an off-curve point.
//
// The SVDW theorem guarantees that at least one of (x1, x2, x3) is a valid
// x-coordinate on the curve, and the final output must be on the curve.
// If the output is NOT on the curve, we've found a bug in SVDW constants,
// sgn0, sign correction, or the syscall oracle.
//
// This specifically targets the concern P02 raised: the BPF try_sqrt_curve
// returns Fq::ZERO for x=0 instead of sqrt(3), which could produce (0, 0)
// as a "valid" point. (0, 0) is NOT on the BN254 curve, so the on-curve
// invariant catches this.
//
// Invariants asserted:
//   1. No panic on any byte input
//   2. Output is always on the BN254 G1 curve (via independent on_curve check)
//   3. Output x and y are canonical (< p)

use alea_verifier::crypto::hash_to_g1::{hash_round_to_g1, hash_to_g1};
use alea_verifier::crypto::pairing::on_curve_g1;
use libfuzzer_sys::fuzz_target;

#[derive(arbitrary::Arbitrary, Debug)]
enum FuzzInput {
    Round(u64),
    RawMessage(Vec<u8>),
}

fuzz_target!(|input: FuzzInput| {
    let point: [u8; 64] = match input {
        FuzzInput::Round(round) => hash_round_to_g1(round),
        FuzzInput::RawMessage(msg) => {
            // Bound message size so we don't starve the fuzzer on multi-block
            // expand_message paths. 1024 bytes is 8x the keccak rate.
            if msg.len() > 1024 {
                return;
            }
            hash_to_g1(&msg)
        }
    };

    // INVARIANT: every point produced by hash-to-curve MUST be on the curve.
    // If this assert fires, SVDW has a correctness bug.
    assert!(
        on_curve_g1(&point),
        "hash_to_g1 produced off-curve point: {:?}",
        hex::encode(&point[..])
    );
});
