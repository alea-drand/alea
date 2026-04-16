use ark_bn254::Fq;
use ark_ff::{BigInteger256, Field, PrimeField, Zero};

use super::constants::{EXPECTED_EVMNET_PUBKEY, G2_GENERATOR, GT_ONE, P_BE, P_BIGINT};
use super::hash_to_g1::hash_round_to_g1;
use super::svdw::{fq_from_be_bytes, fq_to_be_bytes};

/// Validate that bytes decode to a point on BN254 G1 in canonical form
/// Rejects: x >= p, y >= p, or y² != x³ + 3 mod p
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

/// Full BLS verification: verify drand beacon and return randomness
/// Returns 32-byte randomness = sha256(signature) on success, None on failure
pub fn verify_beacon(round: u64, signature: &[u8; 64], pubkey_g2: &[u8; 128]) -> Option<[u8; 32]> {
    // Step 1: validate signature is on curve
    if !on_curve_g1(signature) {
        return None;
    }

    // Step 2: hash round to G1 message point
    let m = hash_round_to_g1(round);

    // Step 3: negate M for pairing check
    let neg_m = negate_g1(&m);

    // Step 4: pairing check e(σ, G2_gen) * e(-M, pubkey) == 1
    if !pairing_check_native(signature, &G2_GENERATOR, &neg_m, pubkey_g2) {
        return None;
    }

    // Step 5: randomness = sha256(signature) — NOT keccak256 (ADR 0036)
    let randomness = anchor_lang::solana_program::hash::hash(signature);
    Some(randomness.to_bytes())
}

/// BN254 pairing check using ark-ec (native)
fn pairing_check_native(
    sigma: &[u8; 64],
    g2_gen: &[u8; 128],
    neg_m: &[u8; 64],
    pubkey: &[u8; 128],
) -> bool {
    use ark_bn254::{Bn254, G1Affine, G2Affine};
    use ark_ec::pairing::Pairing;

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

fn decode_g1(bytes: &[u8; 64]) -> ark_bn254::G1Affine {
    let x = fq_from_be_bytes(bytes[0..32].try_into().unwrap());
    let y = fq_from_be_bytes(bytes[32..64].try_into().unwrap());
    ark_bn254::G1Affine::new_unchecked(x, y)
}

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
}
