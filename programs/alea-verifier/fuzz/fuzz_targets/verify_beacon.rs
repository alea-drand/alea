#![no_main]

// Fuzz target: verify_beacon (end-to-end)
//
// Goal: hunt for any input that causes a panic in the verify_beacon pipeline.
// libFuzzer treats any panic/abort as a crash — that's our primary invariant.
//
// Input: (round: u64, signature: [u8; 64]).
// Pubkey is hardcoded to EXPECTED_EVMNET_PUBKEY because Alea only verifies
// against drand evmnet. Fuzzing random pubkeys would be meaningless for
// Alea's specific trust model.
//
// Expected behavior:
//   - Return Some(randomness) only for the tiny subset of inputs that happen
//     to form valid BLS signatures (probabilistically ~zero in the fuzz budget).
//   - Return None for everything else (wrong sig, off-curve, pairing fail).
//   - NEVER panic on any input.
//
// Invariants asserted below:
//   1. No panic (enforced by libFuzzer runtime)
//   2. On success (Some), output length is exactly 32 bytes (compile-time via type system)
//   3. On success, output == sha256(signature_bytes) (ADR 0036) — deferred check,
//      since finding a valid signature via random fuzzing is 2^-256 probability.

use alea_verifier::crypto::constants::EXPECTED_EVMNET_PUBKEY;
use alea_verifier::crypto::pairing::verify_beacon;
use libfuzzer_sys::fuzz_target;

#[derive(arbitrary::Arbitrary, Debug)]
struct FuzzInput {
    round: u64,
    signature: [u8; 64],
}

fuzz_target!(|input: FuzzInput| {
    // If verify_beacon panics on this input, libFuzzer records it as a crash.
    let _result = verify_beacon(input.round, &input.signature, &EXPECTED_EVMNET_PUBKEY);
    // No assertion needed — panic = crash, non-panic return = pass.
    // Coverage-guided fuzzing will find inputs that exercise new code paths.
});
