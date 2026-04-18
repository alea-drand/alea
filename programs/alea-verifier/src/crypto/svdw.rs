use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, BigInteger, Field, PrimeField, Zero};

use super::constants::{C1, C2, C3, C4, Z};
#[cfg(not(target_os = "solana"))]
use super::constants::B;

/// Convert Fq to 32-byte big-endian representation
pub fn fq_to_be_bytes(fq: &Fq) -> [u8; 32] {
    let bigint = fq.into_bigint();
    let be = bigint.to_bytes_be();
    let mut result = [0u8; 32];
    result.copy_from_slice(&be);
    result
}

/// Convert 32 big-endian bytes to Fq (assumes bytes < p)
pub fn fq_from_be_bytes(bytes: &[u8; 32]) -> Fq {
    Fq::from_be_bytes_mod_order(bytes)
}

/// RFC 9380 §4: sgn0 — returns parity of the canonical integer representation
/// CRITICAL: must use .into_bigint() to escape Montgomery form (T2.22)
pub fn sgn0(x: &Fq) -> u8 {
    let bigint = x.into_bigint();
    if bigint.is_odd() { 1 } else { 0 }
}

/// RFC 9380 §4: inv0 — modular inverse with inv0(0) = 0 convention
pub fn inv0(x: &Fq) -> Fq {
    if x.is_zero() {
        Fq::ZERO
    } else {
        x.inverse().unwrap()
    }
}

// ============================================================================
// Field-op wrappers (T3 — BPF stack-frame management)
// ============================================================================
// Solana BPF has 4KB per call frame + 8 frames max. ark-ff's Fq multiplication
// allocates ~200 bytes internally; 10+ inlined Fq ops in map_to_point blew the
// 4KB frame (measured 5520 bytes pre-fix). `#[inline(never)]` wrappers force
// each op into its own frame so intermediates don't accumulate in the caller.
// See ~/vault/80-learnings/2026-04-16-bpf-stack-frame-management.md.

#[inline(never)]
fn fq_mul(a: &Fq, b: &Fq) -> Fq { *a * *b }

#[inline(never)]
fn fq_sq(a: &Fq) -> Fq { a.square() }

#[inline(never)]
fn fq_add(a: &Fq, b: &Fq) -> Fq { *a + *b }

#[inline(never)]
fn fq_sub(a: &Fq, b: &Fq) -> Fq { *a - *b }

// ============================================================================
// SVDW sqrt oracle — cfg-gated per ADR 0037
// ============================================================================
// Native: ark-ff Fq::sqrt() via Tonelli–Shanks.
// BPF:    alt_bn128_g1_decompress as sqrt oracle (643 CU, Phase 1.1 validated
//         at commit 57aeb8b). Input = 32 BE bytes with sign bit cleared (x < p
//         so byte[0] < 0x30 → MSB already 0, encoding = "positive y"). Output
//         = 64 BE bytes (x || y). Err ⇒ x is not on curve (gx is not a QR) —
//         SVDW falls through to next x-candidate. Sign correction is applied
//         by the caller (map_to_point) regardless of which y was returned.

#[cfg(not(target_os = "solana"))]
fn try_sqrt_curve(x: &Fq) -> Option<Fq> {
    // Stage 9 POSTFIX-T2-01 (P01 Opus) — symmetric zero-x guard.
    // The BPF branch below rejects `x = 0` to close the T1.08 Agave
    // short-circuit divergence. Without the matching guard here, native
    // returns `Some(sqrt(3))` (3 is a QR in Fq since p ≡ 7 mod 12) while
    // BPF returns `None` — asymmetric divergence at x = 0. Reachability
    // under honest drand: 2⁻²⁵⁴ per candidate; polynomial under the new
    // `map_to_point_debug` chosen-input instruction but no forgery path
    // (debug instruction returns raw map_to_point output, not a verified
    // beacon). Mirror the BPF guard so the two paths agree by construction.
    if x.is_zero() {
        return None;
    }
    let gx = fq_add(&fq_mul(&fq_sq(x), x), &B); // x³ + 3
    gx.sqrt()
}

#[cfg(target_os = "solana")]
fn try_sqrt_curve(x: &Fq) -> Option<Fq> {
    use anchor_lang::solana_program::alt_bn128::compression::prelude::alt_bn128_g1_decompress;

    // T1.08 — Explicit zero-x guard. Verified via Agave source
    // (solana-bn254 3.2.1/src/compression.rs:163-164): the syscall
    // short-circuits on all-zero 32-byte input, returning
    // `Ok([0u8; 64])` (the point-at-infinity representation) BEFORE
    // deserialization. Alea's SVDW uses decompress as a sqrt oracle,
    // so this would incorrectly unwrap `(0,0)` as a valid sqrt of
    // g(0)=3 (native path returns sqrt(3), so native/BPF diverge).
    // Reachability under honest drand: ≈ 2⁻²⁵⁴ per candidate; SVDW
    // theorem guarantees at least one of {x1,x2,x3} is a QR, so
    // returning None here falls through to the next candidate —
    // correct SVDW semantics + eliminates the native/BPF divergence
    // + closes the ADR 0037 "no prior art" landmine for downstream
    // forks. Source: P02-T1-02 (Opus); R4 arbitration over P01-T2-01.
    if x.is_zero() {
        return None;
    }

    // x is guaranteed < p, so byte[0] < 0x30 → MSB (sign bit) is already 0
    // (= "positive y" convention in ark_serialize's BN254 compressed form).
    let x_bytes = fq_to_be_bytes(x);
    match alt_bn128_g1_decompress(&x_bytes) {
        Ok(point64) => {
            // point64[32..64] = y in BE
            let mut y_bytes = [0u8; 32];
            y_bytes.copy_from_slice(&point64[32..64]);
            Some(fq_from_be_bytes(&y_bytes))
        }
        Err(_) => None, // x is not on curve (gx is not a QR in Fq)
    }
}

/// SVDW map_to_point: maps a field element to a BN254 G1 point.
/// Port of kevincharm/bls-bn254 BLS.sol mapToPoint().
///
/// Returns 64 big-endian bytes (x || y) on success, or `None` if all
/// three SVDW candidates (x1, x2, x3) fail try_sqrt_curve. Per RFC 9380
/// Appendix F.2.1 + the BN254 SVDW theorem, at least one candidate is
/// always a quadratic residue for any valid `u` — so `None` in practice
/// signals a syscall oracle regression or corrupt SVDW constant (both
/// of which Alea's test suite guards against via fixtures + const
/// sanity checks).
///
/// T1.05 — refactored from `-> [u8; 64]` with `.expect()` on x3 to
/// `-> Option<[u8; 64]>` with `?` propagation. Caller (hash_to_g1) maps
/// None to `AleaError::NoSquareRoot` (6004) via `ok_or`. This converts
/// a BPF panic into a structured on-chain error code.
pub fn map_to_point(u: &Fq) -> Option<[u8; 64]> {
    // Steps 1-4: compute tv1, tv2
    let tv1_inner = fq_mul(&fq_sq(u), &C1); // u² * g(Z)
    let tv2 = fq_add(&Fq::ONE, &tv1_inner); // 1 + u²*g(Z)
    let tv1 = fq_sub(&Fq::ONE, &tv1_inner); // 1 - u²*g(Z)

    // Steps 5-6: tv3 = inv0(tv1 * tv2)
    let tv3 = inv0(&fq_mul(&tv1, &tv2));

    // Step 7: tv5 = u * tv1 * tv3 * C3
    let tv5 = fq_mul(&fq_mul(&fq_mul(u, &tv1), &tv3), &C3);

    // Steps 8-9: candidates x1, x2
    let x1 = fq_sub(&C2, &tv5);
    let x2 = fq_add(&C2, &tv5);

    // Steps 10-12: candidate x3
    let tv7 = fq_sq(&tv2);
    let tv8 = fq_mul(&tv7, &tv3);
    let x3 = fq_add(&Z, &fq_mul(&C4, &fq_sq(&tv8)));

    // Select candidate: try x1, then x2, then x3. If x3 also fails
    // (theorem violation), propagate None to the caller.
    let (selected_x, mut y) = if let Some(y1) = try_sqrt_curve(&x1) {
        (x1, y1)
    } else if let Some(y2) = try_sqrt_curve(&x2) {
        (x2, y2)
    } else {
        let y3 = try_sqrt_curve(&x3)?;
        (x3, y3)
    };

    // Step 19: fix sign — y must have same parity as u
    if sgn0(u) != sgn0(&y) {
        y = -y;
    }

    // Encode as 64 big-endian bytes
    let mut result = [0u8; 64];
    result[0..32].copy_from_slice(&fq_to_be_bytes(&selected_x));
    result[32..64].copy_from_slice(&fq_to_be_bytes(&y));
    Some(result)
}

// ============================================================================
// G1 addition — cfg-gated
// ============================================================================
// Native: ark-ec affine addition (for tests).
// BPF:    alt_bn128_addition syscall (334 CU).
//
// T1.05 — signature changed from `-> [u8; 64]` (with .expect() on BPF
// syscall result) to `-> anchor_lang::prelude::Result<[u8; 64]>` with
// explicit Err propagation. Native path still infallible in practice but
// wrapped in Ok(...) for signature uniformity. BPF syscall Err maps to
// AleaError::PairingError (6006). Converts a potential opaque BPF panic
// into a structured on-chain error code.
//
// Pre-Phase 4 note: `g1_add` is `pub` (part of crypto library surface),
// but not part of ADR 0028 CPI interface contract (which is instruction-
// level). No external consumers exist before Phase 4 SDK publication.

#[cfg(not(target_os = "solana"))]
pub fn g1_add(p1: &[u8; 64], p2: &[u8; 64]) -> anchor_lang::prelude::Result<[u8; 64]> {
    use ark_bn254::G1Affine;
    let x1 = fq_from_be_bytes(p1[0..32].try_into().unwrap());
    let y1 = fq_from_be_bytes(p1[32..64].try_into().unwrap());
    let x2 = fq_from_be_bytes(p2[0..32].try_into().unwrap());
    let y2 = fq_from_be_bytes(p2[32..64].try_into().unwrap());

    let pt1 = G1Affine::new_unchecked(x1, y1);
    let pt2 = G1Affine::new_unchecked(x2, y2);

    let sum: G1Affine = (pt1 + pt2).into();

    let mut result = [0u8; 64];
    result[0..32].copy_from_slice(&fq_to_be_bytes(&sum.x));
    result[32..64].copy_from_slice(&fq_to_be_bytes(&sum.y));
    Ok(result)
}

#[cfg(target_os = "solana")]
pub fn g1_add(p1: &[u8; 64], p2: &[u8; 64]) -> anchor_lang::prelude::Result<[u8; 64]> {
    use anchor_lang::solana_program::alt_bn128::prelude::alt_bn128_addition;
    use crate::errors::AleaError;

    let mut input = [0u8; 128];
    input[0..64].copy_from_slice(p1);
    input[64..128].copy_from_slice(p2);

    let result = alt_bn128_addition(&input)
        .map_err(|_| AleaError::PairingError)?;
    let mut out = [0u8; 64];
    out.copy_from_slice(&result[..64]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sgn0_canonical_not_montgomery() {
        assert_eq!(sgn0(&Fq::from(3u64)), 1, "sgn0(3) must be 1 (odd)");
        assert_eq!(sgn0(&Fq::from(2u64)), 0, "sgn0(2) must be 0 (even)");
        assert_eq!(sgn0(&Fq::from(0u64)), 0, "sgn0(0) = 0");
        assert_eq!(sgn0(&Fq::from(1u64)), 1, "sgn0(1) = 1");

        // p-1 is even (p is odd, p-1 is even)
        let p_minus_1 = -Fq::ONE;
        assert_eq!(sgn0(&p_minus_1), 0, "sgn0(p-1) must be 0 (even)");
    }

    #[test]
    fn inv0_of_zero_is_zero() {
        assert_eq!(inv0(&Fq::ZERO), Fq::ZERO,
            "inv0(0) MUST return 0 per RFC 9380 §4 convention");
    }

    #[test]
    fn inv0_of_nonzero_is_multiplicative_inverse() {
        let x = Fq::from(5u64);
        let inv_x = inv0(&x);
        assert_eq!(inv_x * x, Fq::ONE, "inv0(x) * x MUST equal 1 for nonzero x");
    }

    #[test]
    fn fq_byte_roundtrip() {
        let val = Fq::from(42u64);
        let bytes = fq_to_be_bytes(&val);
        let recovered = fq_from_be_bytes(&bytes);
        assert_eq!(val, recovered);
    }

    #[test]
    fn map_to_point_round_1() {
        let u0 = fq_from_be_bytes(&hex_literal::hex!(
            "1b163e041c11b8ddb908e7b705c98ca4f393243bf3664bf5934a3680d3a5bfc6"
        ));
        let u1 = fq_from_be_bytes(&hex_literal::hex!(
            "0b2f337436437aef114e4f8383ac665c24fe4d3f88b3c53d494ad4104b9d15eb"
        ));

        let q0 = map_to_point(&u0).expect("round 1 u0 must succeed (SVDW theorem)");
        let q1 = map_to_point(&u1).expect("round 1 u1 must succeed (SVDW theorem)");

        // T1.03 — individual Q0 / Q1 byte-for-byte assertions against
        // gnark-crypto fixtures (round-1.json:30-33). The sum assertion
        // below passes for (-Q0, -Q1) and (Q1, Q0) swaps (G1 addition
        // commutative, anti-commutative under negation). These per-point
        // assertions pin branch selection + sign correction individually.
        assert_eq!(hex::encode(&q0[0..32]),
            "1e10b19957a0ab51d8ed02605e5fdb691f78e287817525ed109cb0b5b2519723",
            "Round 1 Q0.x must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q0[32..64]),
            "0742fdfa5dba51b9c799434e73fbb705930d9e29cefad99b31f7255b0d62d370",
            "Round 1 Q0.y must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q1[0..32]),
            "15b1de83d800a488b346a8e46b60404911b9e24f8f0ce295fb1940f2e81fe902",
            "Round 1 Q1.x must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q1[32..64]),
            "21e341fa458ee12634b567e980ff1561fba99ef9e6858e30373b2bb5b3fb2ccf",
            "Round 1 Q1.y must match gnark-crypto MapToG1");

        let m = g1_add(&q0, &q1).expect("g1_add must succeed for on-curve inputs");

        let m_x_hex = hex::encode(&m[0..32]);
        let m_y_hex = hex::encode(&m[32..64]);

        assert_eq!(m_x_hex, "073d3d00a1c3ca588db79d44202e44b2f45995ddd39e705717c9edfcb79e4371",
            "Round 1 M.x must match fixture");
        assert_eq!(m_y_hex, "173e31a5208ea2594cbcb23b5afb3dd930719a4d1a3f877839bb8bdeb3c15084",
            "Round 1 M.y must match fixture");
    }

    #[test]
    fn map_to_point_round_9337227() {
        let u0 = fq_from_be_bytes(&hex_literal::hex!(
            "109ead626603ce780c14be70861676828e42948357c960d53e4250cb47246064"
        ));
        let u1 = fq_from_be_bytes(&hex_literal::hex!(
            "1da61ba0e660ae1d421c04d6aa2a5d69b24a1a1d380d01b464bdf315b080e781"
        ));

        let q0 = map_to_point(&u0).expect("round 9337227 u0 must succeed (SVDW theorem)");
        let q1 = map_to_point(&u1).expect("round 9337227 u1 must succeed (SVDW theorem)");

        // T1.03 — individual Q0 / Q1 byte-for-byte assertions against
        // gnark-crypto fixtures (round-9337227.json:30-33).
        assert_eq!(hex::encode(&q0[0..32]),
            "0bdac09968c4675115f5173ed5a2af9da4dd42dea8d82824cd45d4e40c52f4c3",
            "Round 9337227 Q0.x must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q0[32..64]),
            "1db41b01f6e7a7e1463e4eb6dd35ffd39deca11bf020262592c2f2e3a9e871e2",
            "Round 9337227 Q0.y must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q1[0..32]),
            "2c547cc28601f4c5376d75d935d493dcde85f549ed79c1d136227fa7588a09d8",
            "Round 9337227 Q1.x must match gnark-crypto MapToG1");
        assert_eq!(hex::encode(&q1[32..64]),
            "1116342a64c29038836c8b7b8c1270ca8af9535ca542a0aee6d6b82855157ad3",
            "Round 9337227 Q1.y must match gnark-crypto MapToG1");

        let m = g1_add(&q0, &q1).expect("g1_add must succeed for on-curve inputs");

        let m_x_hex = hex::encode(&m[0..32]);
        let m_y_hex = hex::encode(&m[32..64]);

        assert_eq!(m_x_hex, "1626082c3dd0751bdaaf8c3e709b5dd7cdedf45d4e81a5aa3e270f1e78924b32",
            "Round 9337227 M.x must match fixture");
        assert_eq!(m_y_hex, "2bf29ab3af54dfe3c053ad4efa93db05d3586368463e9d7334c7ba61f6f4e955",
            "Round 9337227 M.y must match fixture");
    }

    #[test]
    fn map_to_point_u_zero() {
        let u = Fq::ZERO;
        let result = map_to_point(&u).expect("u=0 must succeed (one of x_i is a QR by SVDW theorem)");
        let x = fq_from_be_bytes(result[0..32].try_into().unwrap());
        let y = fq_from_be_bytes(result[32..64].try_into().unwrap());
        assert_eq!(y.square(), x.square() * x + Fq::from(3u64), "u=0 result must be on curve");
    }

    #[test]
    fn map_to_point_u_one() {
        let u = Fq::ONE;
        let result = map_to_point(&u).expect("u=1 must succeed");
        let x = fq_from_be_bytes(result[0..32].try_into().unwrap());
        let y = fq_from_be_bytes(result[32..64].try_into().unwrap());
        assert_eq!(y.square(), x.square() * x + Fq::from(3u64), "u=1 result must be on curve");
    }

    #[test]
    fn map_to_point_u_p_minus_1() {
        let u = -Fq::ONE; // p-1
        let result = map_to_point(&u).expect("u=p-1 must succeed");
        let x = fq_from_be_bytes(result[0..32].try_into().unwrap());
        let y = fq_from_be_bytes(result[32..64].try_into().unwrap());
        assert_eq!(y.square(), x.square() * x + Fq::from(3u64), "u=p-1 result must be on curve");
    }

    #[test]
    fn all_svdw_branches_exercised() {
        // Scan first 200 rounds to find inputs that exercise each x-candidate branch
        use anchor_lang::solana_program::keccak;
        use super::super::expand_message::hash_to_field;

        let mut hit_x1 = false;
        let mut hit_x2 = false;
        let mut hit_x3 = false;

        for round in 1..=200u64 {
            let round_bytes = round.to_be_bytes();
            let msg = keccak::hash(&round_bytes);
            let (u0, u1) = hash_to_field(msg.as_ref());

            for u in [u0, u1] {
                let branch = which_branch(&u);
                match branch {
                    1 => hit_x1 = true,
                    2 => hit_x2 = true,
                    3 => hit_x3 = true,
                    _ => unreachable!(),
                }
            }
            if hit_x1 && hit_x2 && hit_x3 {
                break;
            }
        }
        assert!(hit_x1, "x1 branch must be exercised within 200 rounds");
        assert!(hit_x2, "x2 branch must be exercised within 200 rounds");
        assert!(hit_x3, "x3 branch must be exercised within 200 rounds");
    }

    /// Helper: determine which SVDW branch a given u takes
    fn which_branch(u: &Fq) -> u8 {
        let tv1_inner = u.square() * C1;
        let tv2 = Fq::ONE + tv1_inner;
        let tv1 = Fq::ONE - tv1_inner;
        let tv3 = inv0(&(tv1 * tv2));
        let tv5 = *u * tv1 * tv3 * C3;
        let x1 = C2 - tv5;
        let x2 = C2 + tv5;

        let gx1 = x1.square() * x1 + B;
        if gx1.sqrt().is_some() { return 1; }

        let gx2 = x2.square() * x2 + B;
        if gx2.sqrt().is_some() { return 2; }

        3
    }
}
