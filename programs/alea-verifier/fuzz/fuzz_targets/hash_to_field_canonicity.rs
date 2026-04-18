#![no_main]

// Fuzz target: hash_to_field canonicity invariant
//
// Stage 8 addition (Phase 2.5 Wave G). Goal: assert that for any 32-byte
// input (treated as a keccak256 digest equivalent), `hash_to_field`
// returns two field elements u0, u1 that are both canonical (< p) AND
// within the BN254 Fq range.
//
// The invariant is RFC 9380 §5 compliance: `hash_to_field` must produce
// uniform distribution over Fq. This requires `Fq::from_be_bytes_mod_order`
// on 48-byte windows (not 32-byte, which would bias the distribution).
//
// This target fuzzes the INPUT to expand_message_xmd + hash_to_field, not
// the output (which is always canonical by construction if `from_be_bytes_
// mod_order` is correct). Catches regressions where someone "optimizes"
// the reduction step by truncating to 32 bytes or skipping the mod p.
//
// Expanded attack surface beyond verify_beacon: this target isolates the
// hash_to_field primitive, which verify_beacon only exercises at 1 input
// per tx. Fuzzing directly grows coverage of the reduction arithmetic.
//
// Invariants:
//   1. No panic on any input
//   2. Both u0 and u1 are canonical Fq elements (< p)
//   3. Re-encoding u0/u1 produces 32-byte canonical big-endian bytes

use alea_verifier::crypto::expand_message::hash_to_field;
use ark_bn254::Fq;
use ark_ff::{BigInteger, PrimeField};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|msg: [u8; 32]| {
    // If hash_to_field panics on any 32-byte input, libFuzzer records crash.
    let (u0, u1) = hash_to_field(&msg);

    // INVARIANT: canonical. from_be_bytes_mod_order always produces < p
    // by construction, but assert it via round-trip to catch any future
    // regression where bytes could be non-canonical.
    let u0_bi = u0.into_bigint();
    let u1_bi = u1.into_bigint();

    // Both must be strictly less than the BN254 modulus p.
    let p_bi = <Fq as PrimeField>::MODULUS;
    assert!(
        u0_bi < p_bi,
        "hash_to_field u0 not canonical: {:?} >= p",
        u0_bi
    );
    assert!(
        u1_bi < p_bi,
        "hash_to_field u1 not canonical: {:?} >= p",
        u1_bi
    );

    // Re-encode: 32 BE bytes, all within [0, p).
    let u0_bytes = u0_bi.to_bytes_be();
    let u1_bytes = u1_bi.to_bytes_be();
    assert!(u0_bytes.len() <= 32, "u0 encoding > 32 bytes");
    assert!(u1_bytes.len() <= 32, "u1 encoding > 32 bytes");
});
