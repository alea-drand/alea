use ark_bn254::Fq;
use ark_ff::{BigInteger256, Field, PrimeField};

use super::constants::{G2_GENERATOR, P_BIGINT};
#[cfg(target_os = "solana")]
use super::constants::GT_ONE;
#[cfg(test)]
use super::constants::EXPECTED_EVMNET_PUBKEY;
use super::hash_to_g1::hash_round_to_g1;
use super::svdw::{fq_from_be_bytes, fq_to_be_bytes};

/// Validate that bytes decode to a point on BN254 G1 in canonical form.
///
/// Rejects: x >= p, y >= p, or y² != x³ + 3 mod p.
///
/// Runs on both native and BPF via ark-ff field ops (no ark-ec) — ~5
/// field operations per call, acceptable CU cost on BPF.
///
/// # Subgroup check
/// BN254 G1 has **cofactor = 1** — the prime-order subgroup equals the
/// full curve (ark-bn254 defines `COFACTOR = 1` and
/// `is_in_correct_subgroup_assuming_on_curve -> true`). Every on-curve
/// G1 point is automatically in the correct subgroup, so NO explicit
/// subgroup check is needed here (unlike G2, which has a large cofactor
/// — see ADR 0027 fallback path).
///
/// This is critical: do NOT reorder the canonical-form check and the
/// curve equation check. The canonical check (x < p, y < p) MUST run
/// FIRST to reject non-reduced representations that would otherwise
/// pass the curve equation mod p. This is the exact attack shape of
/// CVE-2025-30147 (Besu, May 2025 — subgroup-before-on-curve bypass).
///
/// Sources: T2.X (P02-T2-02 + P03-T3-01). Also documented in PRESERVE.md
/// items PRESERVE-09 + PRESERVE-14.
pub fn on_curve_g1(bytes: &[u8; 64]) -> bool {
    // Parse as big-endian bigints WITHOUT field reduction
    let x_bytes: [u8; 32] = bytes[0..32].try_into().unwrap();
    let y_bytes: [u8; 32] = bytes[32..64].try_into().unwrap();

    let x_bi = bytes_to_bigint(&x_bytes);
    let y_bi = bytes_to_bigint(&y_bytes);

    // Canonical-form rejection: x < p AND y < p
    if x_bi >= P_BIGINT || y_bi >= P_BIGINT {
        return false;
    }

    let x = Fq::from_bigint(x_bi).expect("x < p verified");
    let y = Fq::from_bigint(y_bi).expect("y < p verified");

    // On-curve check: y² == x³ + 3
    y.square() == x.square() * x + Fq::from(3u64)
}

/// Negate a G1 point: (x, y) → (x, p - y)
pub fn negate_g1(point: &[u8; 64]) -> [u8; 64] {
    let y = fq_from_be_bytes(point[32..64].try_into().unwrap());
    let neg_y = -y;
    let mut result = [0u8; 64];
    result[0..32].copy_from_slice(&point[0..32]);
    result[32..64].copy_from_slice(&fq_to_be_bytes(&neg_y));
    result
}

// ============================================================================
// Pairing check — cfg-gated per ADR 0037 + bls-verification.md T2.B2
// ============================================================================
// Public surface: `verify_pairing`.
// Returns:
//   Some(true)  = e(σ, G2_gen) · e(-M, pubkey) == 1_GT  (signature valid)
//   Some(false) = pairing product != 1_GT               (signature invalid)
//   None        = syscall returned Err (BPF infrastructure failure) —
//                 caller maps to AleaError::PairingError per T3.09.
//
// Buffer order (384 bytes) per `program/bls-verification.md §Byte-Layout`:
//   σ(64) || G2_gen(128) || neg_m(64) || pubkey(128)

/// Full BLS verification: verify drand beacon and return randomness.
///
/// T2.O — marked `#[cfg(test)]` so this helper cannot be accidentally
/// called from production code. The Anchor `verify` handler uses the
/// primitives directly so it can emit distinct error codes (6000/6001/
/// 6006) instead of collapsing all failures into `None`.
///
/// T1.05 — hash_round_to_g1 now returns Result; we convert Err to None
/// here to preserve the test-helper's documented "None on any failure"
/// behavior.
#[cfg(test)]
pub fn verify_beacon(round: u64, signature: &[u8; 64], pubkey_g2: &[u8; 128]) -> Option<[u8; 32]> {
    // Step 1: validate signature is on curve
    if !on_curve_g1(signature) {
        return None;
    }

    // Step 2: hash round to G1 message point
    let m = hash_round_to_g1(round).ok()?;

    // Step 3: negate M for pairing check
    let neg_m = negate_g1(&m);

    // Step 4: pairing check e(σ, G2_gen) * e(-M, pubkey) == 1
    match verify_pairing(signature, &neg_m, pubkey_g2, &G2_GENERATOR) {
        Some(true) => {
            // Step 5: randomness = sha256(signature) — NOT keccak256 (ADR 0036)
            let randomness = anchor_lang::solana_program::hash::hash(signature);
            Some(randomness.to_bytes())
        }
        _ => None, // Some(false) = invalid sig; None = syscall error
    }
}

/// Cross-platform pairing check.
///
/// * Native: ark-ec `Bn254::multi_pairing` (used by `cargo test` — see
///   `#[cfg(not(target_os = "solana"))]` helper `pairing_check_native`).
/// * BPF:    Solana `alt_bn128_pairing` syscall (48,485 CU, 2 pairs).
///
/// Argument order follows `program/bls-verification.md §"Pairing Check
/// Details"`: `(sigma, neg_m, pubkey_g2, g2_gen)`. Internally the bytes
/// are assembled as `σ || G2_gen || -M || pubkey` before the syscall /
/// native call.
#[cfg(not(target_os = "solana"))]
pub fn verify_pairing(
    sigma: &[u8; 64],
    neg_m: &[u8; 64],
    pubkey_g2: &[u8; 128],
    g2_gen: &[u8; 128],
) -> Option<bool> {
    Some(pairing_check_native(sigma, g2_gen, neg_m, pubkey_g2))
}

#[cfg(target_os = "solana")]
pub fn verify_pairing(
    sigma: &[u8; 64],
    neg_m: &[u8; 64],
    pubkey_g2: &[u8; 128],
    g2_gen: &[u8; 128],
) -> Option<bool> {
    use anchor_lang::solana_program::alt_bn128::prelude::alt_bn128_pairing;

    let mut input = [0u8; 384];
    input[0..64].copy_from_slice(sigma);
    input[64..192].copy_from_slice(g2_gen);
    input[192..256].copy_from_slice(neg_m);
    input[256..384].copy_from_slice(pubkey_g2);

    // T2.P — explicit match distinguishing Some(true)/Some(false)/None.
    // Length-not-32 (hypothetical future syscall ABI drift) routes to
    // None → caller emits 6006 PairingError. This preserves the tri-state
    // contract: "don't trust the answer, reset caller state" instead of
    // misclassifying as 6000 InvalidSignature ("bad signature, retry").
    match alt_bn128_pairing(&input) {
        Ok(result) if result.len() != 32 => None, // infra surprise → 6006
        Ok(result) if result[..] == GT_ONE[..] => Some(true),
        Ok(_) => Some(false),                      // pairing result != GT_ONE
        Err(_) => None,                            // syscall error → 6006
    }
}

/// Native-only pairing via ark-ec. Not compiled on BPF target — its
/// `final_exponentiation` internal blows the 4KB stack frame and is
/// replaced by `alt_bn128_pairing` syscall on BPF (see `verify_pairing`).
#[cfg(not(target_os = "solana"))]
fn pairing_check_native(
    sigma: &[u8; 64],
    g2_gen: &[u8; 128],
    neg_m: &[u8; 64],
    pubkey: &[u8; 128],
) -> bool {
    use ark_bn254::Bn254;
    use ark_ec::pairing::Pairing;
    use ark_ff::Zero;

    let sig_pt = decode_g1(sigma);
    let m_neg_pt = decode_g1(neg_m);
    let g2_gen_pt = decode_g2(g2_gen);
    let pubkey_pt = decode_g2(pubkey);

    // e(σ, G2_gen) * e(-M, pubkey) == 1
    let result = Bn254::multi_pairing(
        [sig_pt, m_neg_pt],
        [g2_gen_pt, pubkey_pt],
    );
    result.is_zero()
}

#[cfg(not(target_os = "solana"))]
fn decode_g1(bytes: &[u8; 64]) -> ark_bn254::G1Affine {
    let x = fq_from_be_bytes(bytes[0..32].try_into().unwrap());
    let y = fq_from_be_bytes(bytes[32..64].try_into().unwrap());
    ark_bn254::G1Affine::new_unchecked(x, y)
}

#[cfg(not(target_os = "solana"))]
fn decode_g2(bytes: &[u8; 128]) -> ark_bn254::G2Affine {
    use ark_bn254::Fq2;

    // EIP-197 encoding: x_c1 || x_c0 || y_c1 || y_c0
    let x_c1 = fq_from_be_bytes(bytes[0..32].try_into().unwrap());
    let x_c0 = fq_from_be_bytes(bytes[32..64].try_into().unwrap());
    let y_c1 = fq_from_be_bytes(bytes[64..96].try_into().unwrap());
    let y_c0 = fq_from_be_bytes(bytes[96..128].try_into().unwrap());

    let x = Fq2::new(x_c0, x_c1);
    let y = Fq2::new(y_c0, y_c1);
    ark_bn254::G2Affine::new_unchecked(x, y)
}

fn bytes_to_bigint(bytes: &[u8; 32]) -> BigInteger256 {
    // Big-endian bytes to little-endian limbs
    let mut limbs = [0u64; 4];
    for i in 0..4 {
        let offset = 24 - i * 8; // BE: limb 0 is the least significant
        for j in 0..8 {
            limbs[i] |= (bytes[offset + j] as u64) << (56 - j * 8);
        }
    }
    BigInteger256::new(limbs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn round_1_signature_is_on_curve() {
        let sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        assert!(on_curve_g1(&sig), "Round 1 drand signature must be on G1");
    }

    #[test]
    fn x_equals_p_is_rejected() {
        let p_be = hex!("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47");
        let mut bytes = [0u8; 64];
        bytes[0..32].copy_from_slice(&p_be);
        assert!(!on_curve_g1(&bytes), "x = p must be rejected (canonical form)");
    }

    #[test]
    fn y_equals_p_is_rejected() {
        let p_be = hex!("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47");
        let mut bytes = [0u8; 64];
        bytes[32..64].copy_from_slice(&p_be);
        assert!(!on_curve_g1(&bytes), "y = p must be rejected (canonical form)");
    }

    #[test]
    fn known_off_curve_point_is_rejected() {
        let mut bytes = [0u8; 64];
        bytes[31] = 1; // x = 1
        bytes[63] = 1; // y = 1, y² = 1 != x³+3 = 4
        assert!(!on_curve_g1(&bytes), "(x=1, y=1) must be rejected (off curve)");
    }

    #[test]
    fn all_zero_bytes_is_rejected() {
        let bytes = [0u8; 64];
        assert!(!on_curve_g1(&bytes), "(0, 0) must be rejected (off curve)");
    }

    #[test]
    fn negate_g1_correct() {
        let sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        let neg = negate_g1(&sig);
        // x unchanged
        assert_eq!(&neg[0..32], &sig[0..32]);
        // y changed
        assert_ne!(&neg[32..64], &sig[32..64]);
        // double negate = original
        let double_neg = negate_g1(&neg);
        assert_eq!(&double_neg, &sig);
    }

    #[test]
    fn verify_beacon_round_1() {
        let sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        let result = verify_beacon(1, &sig, &EXPECTED_EVMNET_PUBKEY);
        assert!(result.is_some(), "Round 1 verification must succeed");
        let randomness = result.unwrap();
        assert_eq!(
            hex::encode(randomness),
            "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5",
            "Round 1 randomness must match drand API"
        );
    }

    #[test]
    fn verify_beacon_round_9337227() {
        let sig: [u8; 64] = hex!(
            "01d65d6128f4b2df3d08de85543d8efe06b0281d0770246ae3672e8ddd3efda0"
            "269373123458f0b5c0073eeed1c816a06809e127421513e34ee07df6987910b3"
        );
        let result = verify_beacon(9337227, &sig, &EXPECTED_EVMNET_PUBKEY);
        assert!(result.is_some(), "Round 9337227 verification must succeed");
        let randomness = result.unwrap();
        assert_eq!(
            hex::encode(randomness),
            "a1e645cd6193837f626716851f5c42ad4bf63ad75193b2cae40f88c08c8f3bd8",
            "Round 9337227 randomness must match drand API"
        );
    }

    #[test]
    fn verify_beacon_invalid_sig_fails() {
        let mut sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        sig[0] ^= 0xFF; // corrupt the signature
        // Corrupted sig may be off-curve, or pairing may fail
        let result = verify_beacon(1, &sig, &EXPECTED_EVMNET_PUBKEY);
        assert!(result.is_none(), "Invalid signature must fail verification");
    }

    #[test]
    fn verify_beacon_wrong_round_fails() {
        let sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        // Round 1 sig against round 2 — pairing must fail
        let result = verify_beacon(2, &sig, &EXPECTED_EVMNET_PUBKEY);
        assert!(result.is_none(), "Wrong round must fail pairing check");
    }

    #[test]
    fn randomness_is_sha256_not_keccak256() {
        let sig: [u8; 64] = hex!(
            "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
            "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
        );
        let sha256_result = anchor_lang::solana_program::hash::hash(&sig);
        let keccak_result = anchor_lang::solana_program::keccak::hash(&sig);

        assert_eq!(
            hex::encode(sha256_result.to_bytes()),
            "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5",
            "sha256(sig) must match drand randomness"
        );
        assert_ne!(
            hex::encode(keccak_result.as_ref()),
            "781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5",
            "keccak256(sig) must NOT match — ADR 0036"
        );
    }

    #[test]
    #[ignore] // requires network access: cargo test bulk_validation -- --ignored
    fn bulk_validation_110_rounds() {
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct DrandBeacon {
            round: u64,
            signature: String,
            randomness: String,
        }

        let chain_hash = "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";
        let client = reqwest::blocking::Client::new();

        let mut passed = 0;
        let mut failed = 0;

        for round in 1..=110u64 {
            let url = format!("https://api.drand.sh/{}/public/{}", chain_hash, round);
            let resp: DrandBeacon = match client.get(&url).send() {
                Ok(r) => match r.json() {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Round {} JSON parse error: {}", round, e);
                        failed += 1;
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!("Round {} fetch error: {}", round, e);
                    failed += 1;
                    continue;
                }
            };

            assert_eq!(resp.round, round);

            let sig_bytes = hex::decode(&resp.signature).unwrap();
            assert_eq!(sig_bytes.len(), 64, "Round {} sig must be 64 bytes", round);
            let sig: [u8; 64] = sig_bytes.try_into().unwrap();

            let result = verify_beacon(round, &sig, &EXPECTED_EVMNET_PUBKEY);
            assert!(result.is_some(), "Round {} verification failed", round);

            let randomness = result.unwrap();
            assert_eq!(
                hex::encode(randomness),
                resp.randomness,
                "Round {} randomness mismatch: sha256(sig) != drand API",
                round
            );
            passed += 1;
        }

        assert_eq!(failed, 0, "All rounds must succeed (failed: {})", failed);
        assert_eq!(passed, 110, "Must validate exactly 110 rounds");
        eprintln!("Bulk validation: {}/110 rounds passed", passed);
    }
}
