use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, BigInteger256, Field, MontFp};

// SVDW constants for BN254 (Z=1, A=0, B=3)
// Verified against kevincharm/bls-bn254 BLS.sol + independent Python computation
pub const Z: Fq = Fq::ONE;
pub const A: Fq = Fq::ZERO;
pub const B: Fq = MontFp!("3");
pub const C1: Fq = MontFp!("4"); // g(Z) = Z³ + A·Z + B = 4
pub const C2: Fq = MontFp!("10944121435919637611123202872628637544348155578648911831344518947322613104291"); // -Z/2 mod p
pub const C3: Fq = MontFp!("8815841940592487685674414971303048083897117035520822607866"); // sqrt(-12 mod p), sgn0(C3)==0
pub const C4: Fq = MontFp!("7296080957279758407415468581752425029565437052432607887563012631548408736189"); // -16/3 mod p

// BN254 base field prime p (big-endian) for canonical-form rejection
pub const P_BIGINT: BigInteger256 = BigInteger256::new([
    0x3c208c16d87cfd47,
    0x97816a916871ca8d,
    0xb85045b68181585d,
    0x30644e72e131a029,
]);

pub const DST: &[u8] = b"BLS_SIG_BN254G1_XMD:KECCAK-256_SVDW_RO_NUL_";

// GT identity: pairing output equals this iff verification passes (EIP-197 §5)
pub const GT_ONE: [u8; 32] = {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    bytes
};

// BN254 G2 generator (uncompressed, 128 bytes big-endian)
// EIP-197 encoding: x_c1 || x_c0 || y_c1 || y_c0 (imaginary first)
pub const G2_GENERATOR: [u8; 128] = [
    // x_c1 (imaginary)
    0x19, 0x8e, 0x93, 0x93, 0x92, 0x0d, 0x48, 0x3a,
    0x72, 0x60, 0xbf, 0xb7, 0x31, 0xfb, 0x5d, 0x25,
    0xf1, 0xaa, 0x49, 0x33, 0x35, 0xa9, 0xe7, 0x12,
    0x97, 0xe4, 0x85, 0xb7, 0xae, 0xf3, 0x12, 0xc2,
    // x_c0 (real)
    0x18, 0x00, 0xde, 0xef, 0x12, 0x1f, 0x1e, 0x76,
    0x42, 0x6a, 0x00, 0x66, 0x5e, 0x5c, 0x44, 0x79,
    0x67, 0x43, 0x22, 0xd4, 0xf7, 0x5e, 0xda, 0xdd,
    0x46, 0xde, 0xbd, 0x5c, 0xd9, 0x92, 0xf6, 0xed,
    // y_c1 (imaginary)
    0x09, 0x06, 0x89, 0xd0, 0x58, 0x5f, 0xf0, 0x75,
    0xec, 0x9e, 0x99, 0xad, 0x69, 0x0c, 0x33, 0x95,
    0xbc, 0x4b, 0x31, 0x33, 0x70, 0xb3, 0x8e, 0xf3,
    0x55, 0xac, 0xda, 0xdc, 0xd1, 0x22, 0x97, 0x5b,
    // y_c0 (real)
    0x12, 0xc8, 0x5e, 0xa5, 0xdb, 0x8c, 0x6d, 0xeb,
    0x4a, 0xab, 0x71, 0x80, 0x8d, 0xcb, 0x40, 0x8f,
    0xe3, 0xd1, 0xe7, 0x69, 0x0c, 0x43, 0xd3, 0x7b,
    0x4c, 0xe6, 0xcc, 0x01, 0x66, 0xfa, 0x7d, 0xaa,
];

// drand evmnet G2 public key (128 bytes, uncompressed)
pub const EXPECTED_EVMNET_PUBKEY: [u8; 128] = [
    0x07, 0xe1, 0xd1, 0xd3, 0x35, 0xdf, 0x83, 0xfa,
    0x98, 0x46, 0x20, 0x05, 0x69, 0x03, 0x72, 0xc6,
    0x43, 0x34, 0x00, 0x60, 0xd2, 0x05, 0x30, 0x6a,
    0x9a, 0xa8, 0x10, 0x6b, 0x6b, 0xd0, 0xb3, 0x82,
    0x05, 0x57, 0xec, 0x32, 0xc2, 0xad, 0x48, 0x8e,
    0x4d, 0x4f, 0x60, 0x08, 0xf8, 0x9a, 0x34, 0x6f,
    0x18, 0x49, 0x20, 0x92, 0xcc, 0xc0, 0xd5, 0x94,
    0x61, 0x0d, 0xe2, 0x73, 0x2c, 0x8b, 0x80, 0x8f,
    0x00, 0x95, 0x68, 0x5a, 0xe3, 0xa8, 0x5b, 0xa2,
    0x43, 0x74, 0x7b, 0x1b, 0x2f, 0x42, 0x60, 0x49,
    0x01, 0x0f, 0x6b, 0x73, 0xa0, 0xcf, 0x1d, 0x38,
    0x93, 0x51, 0xd5, 0xaa, 0xaa, 0x10, 0x47, 0xf6,
    0x29, 0x7d, 0x3a, 0x4f, 0x97, 0x49, 0xb3, 0x3e,
    0xb2, 0xd9, 0x04, 0xc9, 0xd9, 0xeb, 0xf1, 0x72,
    0x24, 0x15, 0x0d, 0xdd, 0x7a, 0xbd, 0x75, 0x67,
    0xa9, 0xbe, 0xc6, 0xc7, 0x44, 0x80, 0xee, 0x0b,
];

// drand evmnet chain hash (32 bytes)
pub const EXPECTED_EVMNET_CHAIN_HASH: [u8; 32] = [
    0x04, 0xf1, 0xe9, 0x06, 0x2b, 0x8a, 0x81, 0xf8,
    0x48, 0xfd, 0xed, 0x9c, 0x12, 0x30, 0x67, 0x33,
    0x28, 0x2b, 0x27, 0x27, 0xec, 0xce, 0xd5, 0x00,
    0x32, 0x18, 0x77, 0x51, 0x16, 0x6e, 0xc8, 0xc3,
];

// BN254 base field prime p (big-endian bytes) for big_mod_exp fallback
pub const P_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
    0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d,
    0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::{BigInteger, Field, PrimeField};

    #[test]
    fn fq_basic_ops() {
        let x = Fq::from(42u64);
        let y = x.square();
        assert_ne!(y, Fq::ZERO);
        assert_eq!(x * x, y);
    }

    #[test]
    fn c3_squared_equals_neg_12() {
        let c3_sq = C3.square();
        let neg_12: Fq = -Fq::from(12u64);
        assert_eq!(c3_sq, neg_12, "C3² must equal -12 mod p");
    }

    // T1.02 — sgn0(C3) == 0 const-sanity. Both C3 and p-C3 square to -12;
    // without this test, a stale copy-paste or refactor could swap them
    // silently and every SVDW tv5 term would flip sign, swapping x1 ↔ x2.
    // RFC 9380 §8.9.1 mandates sgn0(C3) == 0 (smaller-root convention).
    #[test]
    fn c3_sgn0_is_zero() {
        let c3_bigint = C3.into_bigint();
        assert_eq!(
            c3_bigint.0[0] & 1,
            0,
            "C3 MUST be the sqrt(-12) root with sgn0=0 (even). \
             If this fails, C3 may have been replaced with p - C3."
        );
    }

    #[test]
    fn c2_equals_neg_half() {
        let two = Fq::from(2u64);
        let expected = -two.inverse().unwrap();
        assert_eq!(C2, expected, "C2 must equal -Z/2 mod p (with Z=1)");
    }

    #[test]
    fn c4_equals_neg_16_over_3() {
        let sixteen = Fq::from(16u64);
        let three = Fq::from(3u64);
        let expected = -sixteen * three.inverse().unwrap();
        assert_eq!(C4, expected, "C4 must equal -16/3 mod p");
    }

    #[test]
    fn dst_length_invariants() {
        assert_eq!(DST.len(), 43, "DST must be exactly 43 ASCII bytes");
        assert_eq!(
            DST,
            b"BLS_SIG_BN254G1_XMD:KECCAK-256_SVDW_RO_NUL_",
            "DST literal must match drand evmnet bls-bn254-unchained-on-g1 scheme"
        );

        let mut dst_prime = DST.to_vec();
        dst_prime.push(43u8);
        assert_eq!(dst_prime.len(), 44, "DST_prime must be 44 bytes after length byte append");
        assert_eq!(dst_prime[43], 0x2B, "length byte must encode 43 (0x2B)");
    }

    #[test]
    fn gt_one_is_eip197_true() {
        assert_eq!(GT_ONE[31], 1);
        assert!(GT_ONE[..31].iter().all(|&b| b == 0));
    }

    #[test]
    fn g2_generator_length() {
        assert_eq!(G2_GENERATOR.len(), 128);
    }

    #[test]
    fn evmnet_pubkey_length() {
        assert_eq!(EXPECTED_EVMNET_PUBKEY.len(), 128);
    }

    #[test]
    fn evmnet_chain_hash_length() {
        assert_eq!(EXPECTED_EVMNET_CHAIN_HASH.len(), 32);
    }

    // T2.W upgrade — pin against canonical BN254 base field prime, not just
    // self-consistency between our two representations. Previous test only
    // asserted P_BIGINT.to_bytes_be() == P_BE, which would pass even if both
    // constants were corrupted identically (e.g., cloned from a different
    // curve). Now three assertions:
    //  (1) P_BIGINT equals ark-bn254's canonical <Fq as PrimeField>::MODULUS
    //  (2) P_BE equals EIP-197 §5 canonical hex
    //  (3) Cross-consistency preserved
    #[test]
    fn p_bigint_matches_p_be() {
        // (1) P_BIGINT matches ground-truth canonical BN254 modulus
        assert_eq!(
            P_BIGINT,
            <Fq as PrimeField>::MODULUS,
            "P_BIGINT must equal <Fq as PrimeField>::MODULUS (canonical BN254 base field prime)"
        );

        // (2) P_BE matches EIP-197 §5 canonical encoding
        // p = 21888242871839275222246405745257275088696311157297823662689037894645226208583
        // hex: 30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47
        let expected_p_be: [u8; 32] = hex::decode(
            "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47",
        )
        .unwrap()
        .try_into()
        .unwrap();
        assert_eq!(
            P_BE, expected_p_be,
            "P_BE must match EIP-197 §5 canonical hex encoding of BN254 p"
        );

        // (3) Cross-consistency: the two representations encode the same integer
        let p_bytes = P_BIGINT.to_bytes_be();
        assert_eq!(
            p_bytes, P_BE,
            "P_BIGINT.to_bytes_be() must equal P_BE byte array"
        );
    }
}
