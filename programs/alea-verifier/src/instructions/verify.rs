use anchor_lang::prelude::*;

use crate::crypto::{
    constants::G2_GENERATOR,
    hash_to_g1::hash_round_to_g1,
    pairing::{negate_g1, on_curve_g1, verify_pairing},
};
use crate::errors::AleaError;
use crate::events::BeaconVerified;
use crate::state::Config;

/// Accounts for the `verify` instruction.
///
/// `bump = config.bump` reuses the stored canonical bump instead of
/// re-deriving (≈10K CU saving per ADR 0028).
///
/// `payer` (T2.27 rename from `verifier`) is the tx signer that funds
/// the verification. Emitted in `BeaconVerified` for analytics; privacy
/// note in `program/spec.md §"Privacy Considerations"`.
#[derive(Accounts)]
pub struct Verify<'info> {
    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    pub payer: Signer<'info>,
}

/// Pure BLS verification pipeline — no Anchor context dependency.
///
/// Factored out of `verify_handler` so it can be unit-tested natively
/// (round-1 + round-9337227 drand fixtures, round-0 guard, corrupt sig,
/// non-canonical G1 encoding). The Anchor handler is a thin emit-wrapper.
///
/// Returns the 32-byte randomness on success; mapped to
/// `Anchor::Result<[u8; 32]>` error codes per `program/spec.md §"Error
/// Codes"` and §"Error Handling Details" (T3.09 tri-state for pairing).
fn verify_beacon_full(
    round: u64,
    signature: &[u8; 64],
    pubkey_g2: &[u8; 128],
) -> Result<[u8; 32]> {
    // SECURITY: guard ordering is load-bearing. DO NOT REORDER.
    // 1. round > 0 (cheapest; protects against drand genesis sentinel)
    // 2. on_curve_g1 (canonical-form check BEFORE curve equation — CVE-
    //    2025-30147 parallel pattern: subgroup/curve ordering inversion
    //    allows bypass)
    // 3. hash_round_to_g1 (pure; no attacker input reaches this)
    // 4. pairing (only runs if prior guards passed)
    // T2.Y — `config.pubkey_g2 == EXPECTED_EVMNET_PUBKEY` defense-in-
    // depth considered and deliberately skipped: invariant already holds
    // via ADR 0028 PDA-singleton + init-time guards; +200 CU per verify
    // not justified by current attack surface. Reference: cross-model-
    // delta.md + R3 decision #13.
    require!(round > 0, AleaError::RoundZero);                                 // 6002
    require!(on_curve_g1(signature), AleaError::InvalidG1Point);               // 6001

    // msg_hash = keccak256(round.to_be_bytes()) happens inside hash_round_to_g1
    // (T1.02/T1.03: drand signs H2C(keccak256(8-byte BE round)))
    // T1.05 — hash_round_to_g1 now returns Result; None from map_to_point
    // maps to AleaError::NoSquareRoot (6004), Err from g1_add syscall maps
    // to AleaError::PairingError (6006). ? propagates both.
    let m = hash_round_to_g1(round)?;

    // T2.I — defense-in-depth: SVDW + hash_to_field + g1_add must produce
    // on-curve output. debug_assert compiles out in release (zero CU cost)
    // but catches any refactor-introduced regression in tests.
    debug_assert!(on_curve_g1(&m), "SVDW invariant violated: hash_round_to_g1 returned off-curve point");

    let neg_m = negate_g1(&m);

    match verify_pairing(signature, &neg_m, pubkey_g2, &G2_GENERATOR) {
        Some(true) => {
            // randomness = sha256(signature) — NOT keccak256 (ADR 0036).
            // drand evmnet `bls-bn254-unchained-on-g1` scheme; verified
            // empirically against live API rounds 1 + 9337227.
            let randomness = anchor_lang::solana_program::hash::hash(signature).to_bytes();
            Ok(randomness)
        }
        Some(false) => Err(AleaError::InvalidSignature.into()),                // 6000
        None => Err(AleaError::PairingError.into()),                           // 6006 (BPF syscall Err only)
    }
}

/// `verify` handler — wires the crypto pipeline into Anchor return data.
///
/// `Ok([u8; 32])` return type instructs Anchor 0.30.x to auto-serialize
/// the randomness into program return data (ADR 0030 — Pattern A, empirical
/// confirmation deferred to Phase 2 Wave 10 test #12).
pub fn verify_handler(
    ctx: Context<Verify>,
    round: u64,
    signature: [u8; 64],
) -> Result<[u8; 32]> {
    let randomness = verify_beacon_full(round, &signature, &ctx.accounts.config.pubkey_g2)?;

    emit!(BeaconVerified {
        round,
        randomness,
        payer: ctx.accounts.payer.key(),
    });

    Ok(randomness)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::constants::EXPECTED_EVMNET_PUBKEY;
    use hex_literal::hex;

    /// Extract the numeric error code from an Anchor `Error`. Panics on
    /// non-AnchorError variants since all AleaError codes go through
    /// the `#[error_code]` macro.
    fn err_code(err: anchor_lang::error::Error) -> u32 {
        match err {
            anchor_lang::error::Error::AnchorError(ae) => ae.error_code_number,
            other => panic!("expected AnchorError, got {other:?}"),
        }
    }

    const ROUND_1_SIG: [u8; 64] = hex!(
        "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
        "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
    );

    const ROUND_9337227_SIG: [u8; 64] = hex!(
        "01d65d6128f4b2df3d08de85543d8efe06b0281d0770246ae3672e8ddd3efda0"
        "269373123458f0b5c0073eeed1c816a06809e127421513e34ee07df6987910b3"
    );

    #[test]
    fn verify_round_1_fixture_produces_drand_randomness() {
        let randomness = verify_beacon_full(1, &ROUND_1_SIG, &EXPECTED_EVMNET_PUBKEY)
            .expect("round 1 must verify");
        assert_eq!(
            hex::encode(randomness),
            "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5",
            "round 1 randomness must match drand API fixture"
        );
    }

    #[test]
    fn verify_round_9337227_fixture_produces_drand_randomness() {
        let randomness = verify_beacon_full(9337227, &ROUND_9337227_SIG, &EXPECTED_EVMNET_PUBKEY)
            .expect("round 9337227 must verify");
        assert_eq!(
            hex::encode(randomness),
            "a1e645cd6193837f626716851f5c42ad4bf63ad75193b2cae40f88c08c8f3bd8",
            "round 9337227 randomness must match drand API fixture (randa-mu test vector)"
        );
    }

    #[test]
    fn verify_round_zero_returns_6002() {
        let err = verify_beacon_full(0, &ROUND_1_SIG, &EXPECTED_EVMNET_PUBKEY)
            .expect_err("round 0 must fail");
        assert_eq!(err_code(err), 6002, "round 0 must return AleaError::RoundZero");
    }

    #[test]
    fn verify_non_canonical_g1_x_equals_p_returns_6001() {
        // x = p (non-canonical — field element encoding is invalid)
        let p_be = hex!("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47");
        let mut sig = [0u8; 64];
        sig[0..32].copy_from_slice(&p_be);
        // y bytes can be anything — on_curve_g1 rejects at the canonical-form
        // gate before looking at y
        let err = verify_beacon_full(1, &sig, &EXPECTED_EVMNET_PUBKEY)
            .expect_err("non-canonical x must fail");
        assert_eq!(err_code(err), 6001, "x=p must return AleaError::InvalidG1Point");
    }

    #[test]
    fn verify_off_curve_signature_returns_6001() {
        // (x=1, y=1) is off curve: y² = 1 ≠ x³ + 3 = 4
        let mut sig = [0u8; 64];
        sig[31] = 1; // x = 1
        sig[63] = 1; // y = 1
        let err = verify_beacon_full(1, &sig, &EXPECTED_EVMNET_PUBKEY)
            .expect_err("off-curve sig must fail");
        assert_eq!(err_code(err), 6001, "off-curve sig must return AleaError::InvalidG1Point");
    }

    #[test]
    fn verify_corrupt_signature_bit_flip_rejected() {
        // Start with a valid on-curve sig, flip a single bit. The corrupted
        // sig may still land on curve but will fail pairing, OR may go
        // off-curve and fail on_curve_g1 first. Either way: not 6002, not Ok.
        let mut sig = ROUND_1_SIG;
        sig[0] ^= 0xFF;
        let err = verify_beacon_full(1, &sig, &EXPECTED_EVMNET_PUBKEY)
            .expect_err("corrupt sig must fail");
        let code = err_code(err);
        assert!(
            code == 6000 || code == 6001,
            "corrupt sig must return InvalidSignature (6000) or InvalidG1Point (6001), got {code}"
        );
    }

    #[test]
    fn verify_wrong_round_returns_6000() {
        // round 1 signature presented under round 2 — on curve but pairing fails
        let err = verify_beacon_full(2, &ROUND_1_SIG, &EXPECTED_EVMNET_PUBKEY)
            .expect_err("wrong round must fail pairing");
        assert_eq!(err_code(err), 6000, "wrong round must return AleaError::InvalidSignature");
    }
}
