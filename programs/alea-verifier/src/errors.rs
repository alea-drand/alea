use anchor_lang::prelude::*;

/// Alea error codes. Codes 6000-6009 are assigned in declaration order
/// and are part of the v1 CPI interface per ADR 0028 — never renumber,
/// never remove (reserved even if unreachable, see NoSquareRoot).
///
/// Canonical source: `build-spec/program/spec.md §"Error Codes"`.
/// Consumer SDKs (TS `@alea/sdk` and Rust `alea-sdk`) map these 1:1.
///
/// # Anchor framework error codes to be aware of (not Alea-custom)
///
/// T2.AA (Codex C LOW) — Anchor 0.30.1 emits distinct framework codes
/// for different failure modes on `UpdateConfig`. Consumers and
/// monitoring must handle both:
///
///   - **2001 `ConstraintHasOne`**: `has_one = authority` mismatch —
///     the `authority` account's pubkey does not equal `config.authority`.
///     Fires AFTER account deserialization, BEFORE handler body.
///
///   - **3010 `AccountNotSigner`**: `authority: Signer<'info>` present
///     but the account was passed without a signature. Fires during
///     account resolution, EARLIER than 2001.
///
/// These are distinct failure modes: wrong key (2001) vs no signature
/// (3010). Tests + monitoring should not conflate them.
#[error_code]
pub enum AleaError {
    #[msg("BLS signature verification failed")]
    InvalidSignature, // 6000 — alt_bn128_pairing Ok but result != GT_ONE

    #[msg("Signature bytes are not a valid G1 point (y² != x³ + 3 mod p)")]
    InvalidG1Point, // 6001 — pre-pairing on_curve_g1 check failed (T2.48)

    #[msg("Round number must be greater than 0")]
    RoundZero, // 6002 — drand has no valid beacon for round 0

    #[msg("Field element is not in the valid range")]
    InvalidFieldElement, // 6003 — T2.J: reserved per ADR 0028;
    //  currently unreachable (hash_to_field uses
    //  from_be_bytes_mod_order, so no range check
    //  needed). Retained for future use if a
    //  future instruction adds a canonical-Fq
    //  guard. Consumers may ignore.
    #[msg("Square root does not exist for this field element")]
    NoSquareRoot, // 6004 — T1.05: ACTIVATED. Emitted when
    //  hash_round_to_g1 returns None (all 3
    //  SVDW candidates fail try_sqrt_curve).
    //  SVDW theorem guarantees at least one
    //  is a QR, so in practice this signals
    //  a constant corruption or syscall oracle
    //  regression (not attacker-reachable
    //  under honest drand input).
    #[msg("Public key bytes are not a valid G2 point")]
    InvalidG2Point, // 6005 — UNREACHABLE under ADR 0027 fallback
    //  path (byte-for-byte hardcoded pubkey is
    //  strictly stronger than subgroup check
    //  for this single-chain deployment).
    //  Reserved per ADR 0028 CPI stability.
    #[msg("Pairing check syscall failed")]
    PairingError, // 6006 — alt_bn128_pairing returned Err OR
    //  output length != 32 (tri-state None
    //  branch). T1.05: g1_add also maps here
    //  when alt_bn128_addition syscall fails.
    #[msg("chain_hash does not match EXPECTED_EVMNET_CHAIN_HASH (wrong-chain deployment)")]
    WrongChainHash, // 6007 — ADR 0031 chain-hash guard

    #[msg("pubkey_g2 does not match EXPECTED_EVMNET_G2_PUBKEY (ADR 0027 fallback path)")]
    WrongPubkey, // 6008 — fallback G2 validation path (OPEN-ITEMS #4 RESOLVED)

    #[msg("CPI consumer: get_return_data returned None (program upgrade mismatch?)")]
    ReturnDataMissing, // 6009 — UNREACHABLE under Pattern A (ADR
    //  0030 resolved Phase 2 Wave 10 P0#12).
    //  Retained per ADR 0028 CPI stability.
    #[msg("genesis_time does not match EXPECTED_EVMNET_GENESIS_TIME")]
    InvalidGenesisTime, // 6010 — T2.E (Wave E): byte-equality guard
    //  against evmnet constant. Appended per
    //  ADR 0028 append-only rule.
    #[msg("period does not match EXPECTED_EVMNET_PERIOD")]
    InvalidPeriod, // 6011 — T2.E (Wave E): byte-equality guard
    //  against evmnet constant. Appended per
    //  ADR 0028 append-only rule.
    #[msg("initialize authority must equal the program's upgrade_authority_address")]
    UnauthorizedInit, // 6012 — Wave X+1 (Codex Session C HIGH,
                      //  2026-04-17): FENDER-002 hardening at
                      //  the program level. The `initialize`
                      //  handler requires the signer's pubkey
                      //  to equal the BPFLoaderUpgradeable
                      //  ProgramData.upgrade_authority_address,
                      //  closing the deploy-to-init front-run
                      //  window that the prior `scripts/
                      //  initialize.ts` operational mitigation
                      //  could not close (Solana cannot bundle
                      //  program deploy + first-init in a
                      //  single tx — deploy is itself multi-tx
                      //  upload). Appended per ADR 0028
                      //  append-only rule.
}
