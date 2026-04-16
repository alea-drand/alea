use anchor_lang::prelude::*;

/// Alea error codes. Codes 6000-6009 are assigned in declaration order
/// and are part of the v1 CPI interface per ADR 0028 — never renumber,
/// never remove (reserved even if unreachable, see NoSquareRoot).
///
/// Canonical source: `build-spec/program/spec.md §"Error Codes"`.
/// Consumer SDKs (TS `@alea/sdk` and Rust `alea-sdk`) map these 1:1.
///
/// Anchor's `has_one = authority` on `UpdateConfig` emits code 2001
/// (`ConstraintHasOne`) automatically on signer mismatch — not a
/// custom variant here (T1.06 consolidation).
#[error_code]
pub enum AleaError {
    #[msg("BLS signature verification failed")]
    InvalidSignature,           // 6000 — alt_bn128_pairing Ok but result != GT_ONE

    #[msg("Signature bytes are not a valid G1 point (y² != x³ + 3 mod p)")]
    InvalidG1Point,             // 6001 — pre-pairing on_curve_g1 check failed (T2.48)

    #[msg("Round number must be greater than 0")]
    RoundZero,                  // 6002 — drand has no valid beacon for round 0

    #[msg("Field element is not in the valid range")]
    InvalidFieldElement,        // 6003 — hash_to_field / map_to_point range check

    #[msg("Square root does not exist for this field element")]
    NoSquareRoot,               // 6004 — defensive only; unreachable per SVDW theorem

    #[msg("Public key bytes are not a valid G2 point")]
    InvalidG2Point,             // 6005 — ADR 0027 primary path only (subgroup check)

    #[msg("Pairing check syscall failed")]
    PairingError,               // 6006 — alt_bn128_pairing returned Err (infrastructure)

    #[msg("chain_hash does not match EXPECTED_EVMNET_CHAIN_HASH (wrong-chain deployment)")]
    WrongChainHash,             // 6007 — ADR 0031 chain-hash guard

    #[msg("pubkey_g2 does not match EXPECTED_EVMNET_G2_PUBKEY (ADR 0027 fallback path)")]
    WrongPubkey,                // 6008 — fallback G2 validation path (OPEN-ITEMS #4 RESOLVED)

    #[msg("CPI consumer: get_return_data returned None (program upgrade mismatch?)")]
    ReturnDataMissing,          // 6009 — only active if ADR 0030 picks manual pattern
}
