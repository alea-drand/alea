use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;

use super::expand_message::hash_to_field;
use super::svdw::{g1_add, map_to_point};
use crate::errors::AleaError;

/// Hash a message to a BN254 G1 point using SVDW (RFC 9380).
///
/// Input: 32-byte message (keccak256 hash of round number).
/// Output: 64 big-endian bytes (x || y) of G1 affine point.
///
/// T1.05 — returns `Result` (was `[u8; 64]` infallible). Propagates:
/// - `map_to_point` None → `AleaError::NoSquareRoot` (6004): SVDW theorem
///   violation (constant corruption / syscall oracle regression)
/// - `g1_add` Err → `AleaError::PairingError` (6006): BPF syscall infra
///   failure (forwarded by `g1_add` itself)
pub fn hash_to_g1(msg: &[u8]) -> Result<[u8; 64]> {
    let (u0, u1) = hash_to_field(msg);
    let q0 = map_to_point(&u0).ok_or(AleaError::NoSquareRoot)?;
    let q1 = map_to_point(&u1).ok_or(AleaError::NoSquareRoot)?;
    g1_add(&q0, &q1)
}

/// Hash a drand round number to a G1 point.
///
/// Computes keccak256(round.to_be_bytes()) then hash_to_g1.
/// T1.05 — returns `Result` (forwards from hash_to_g1).
pub fn hash_round_to_g1(round: u64) -> Result<[u8; 64]> {
    let round_bytes = round.to_be_bytes();
    let msg_hash = keccak::hash(&round_bytes);
    hash_to_g1(msg_hash.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_to_g1_round_1() {
        let msg_hash =
            hex::decode("6c31fc15422ebad28aaf9089c306702f67540b53c7eea8b7d2941044b027100f")
                .unwrap();
        let m = hash_to_g1(&msg_hash).expect("round 1 must succeed");
        assert_eq!(
            hex::encode(&m[0..32]),
            "073d3d00a1c3ca588db79d44202e44b2f45995ddd39e705717c9edfcb79e4371"
        );
        assert_eq!(
            hex::encode(&m[32..64]),
            "173e31a5208ea2594cbcb23b5afb3dd930719a4d1a3f877839bb8bdeb3c15084"
        );
    }

    #[test]
    fn hash_to_g1_round_9337227() {
        let msg_hash =
            hex::decode("baf09720c37cb921fd8362b1d907232ac0b813ffba768c714aeaace987e7fd6b")
                .unwrap();
        let m = hash_to_g1(&msg_hash).expect("round 9337227 must succeed");
        assert_eq!(
            hex::encode(&m[0..32]),
            "1626082c3dd0751bdaaf8c3e709b5dd7cdedf45d4e81a5aa3e270f1e78924b32"
        );
        assert_eq!(
            hex::encode(&m[32..64]),
            "2bf29ab3af54dfe3c053ad4efa93db05d3586368463e9d7334c7ba61f6f4e955"
        );
    }

    #[test]
    fn hash_round_to_g1_e2e_round_1() {
        let m = hash_round_to_g1(1).expect("round 1 must succeed");
        assert_eq!(
            hex::encode(&m[0..32]),
            "073d3d00a1c3ca588db79d44202e44b2f45995ddd39e705717c9edfcb79e4371"
        );
        assert_eq!(
            hex::encode(&m[32..64]),
            "173e31a5208ea2594cbcb23b5afb3dd930719a4d1a3f877839bb8bdeb3c15084"
        );
    }

    #[test]
    fn hash_round_to_g1_e2e_round_9337227() {
        let m = hash_round_to_g1(9337227).expect("round 9337227 must succeed");
        assert_eq!(
            hex::encode(&m[0..32]),
            "1626082c3dd0751bdaaf8c3e709b5dd7cdedf45d4e81a5aa3e270f1e78924b32"
        );
        assert_eq!(
            hex::encode(&m[32..64]),
            "2bf29ab3af54dfe3c053ad4efa93db05d3586368463e9d7334c7ba61f6f4e955"
        );
    }

    // P10-T3-04 (Phase 2.5 Wave I, Bucket A) — round-bytes endianness pin.
    // drand evmnet signs keccak256(round_as_u64_be_bytes). An accidental
    // switch to little-endian (.to_le_bytes) would produce a completely
    // different keccak hash, completely different SVDW output, completely
    // different G1 point — and a non-verifying signature. Test pins the
    // BE convention explicitly.
    #[test]
    fn hash_round_to_g1_uses_big_endian_round_bytes() {
        // Round 1 u64 big-endian = 00 00 00 00 00 00 00 01
        // keccak256 of that = 6c31fc15422ebad28aaf9089c306702f67540b53c7eea8b7d2941044b027100f
        // Cross-verified against gnark-crypto's hash-to-curve implementation.
        let round_bytes = 1u64.to_be_bytes();
        assert_eq!(
            round_bytes,
            [0, 0, 0, 0, 0, 0, 0, 1],
            "round 1 BE bytes must be [0,0,0,0,0,0,0,1] (LE would be [1,0,0,0,0,0,0,0])"
        );

        let msg_hash = anchor_lang::solana_program::keccak::hash(&round_bytes);
        assert_eq!(
            hex::encode(msg_hash.as_ref()),
            "6c31fc15422ebad28aaf9089c306702f67540b53c7eea8b7d2941044b027100f",
            "keccak256(1_u64_be) must match fixture; LE encoding would produce a different hash",
        );

        // Sanity: the same round via the public API produces a G1 point
        // matching the fixture's hash_to_curve output (end-to-end BE check).
        let m = hash_round_to_g1(1).expect("round 1 must succeed");
        assert_eq!(
            hex::encode(&m[0..32]),
            "073d3d00a1c3ca588db79d44202e44b2f45995ddd39e705717c9edfcb79e4371",
            "round 1 M.x via BE path must match fixture"
        );
    }
}
