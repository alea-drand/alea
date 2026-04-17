use anchor_lang::solana_program::keccak;
use ark_bn254::Fq;
use ark_ff::PrimeField;

use super::constants::DST;

/// RFC 9380 §5.3.1 expand_message_xmd with keccak256.
///
/// Output length is fixed at 96 bytes — sufficient for two BN254 field
/// element draws of 48 bytes each (RFC 9380 §5.3 `L = ceil((ceil(log2(p))
/// + k) / 8)` with k=128-bit security target). Hardcoding in the return
/// type eliminates the pre-T1.05 `assert_eq!(len_in_bytes, 96)` panic and
/// the `len_in_bytes as u16` silent-truncation (CLIPPY-016).
///
/// DST length is constrained by RFC 9380 §5.3.3 to < 256 bytes; Alea's
/// sole caller passes the compile-time `DST` constant (43 bytes — test
/// `dst_length_invariants` pins this). T2.M — `dst_prime` is now a
/// stack-allocated [u8; 44] instead of Vec::with_capacity, eliminating
/// the one heap allocation from the verify hot path.
///
/// Sources: T1.05 (P04/P05/P09 unified panic removal), T2.M (P04),
/// CLIPPY-015/016 (Stage 1 static analysis).
pub fn expand_message_xmd(msg: &[u8], dst: &[u8]) -> [u8; 96] {
    // Precondition: caller-supplied DST is < 256 bytes. Alea's internal
    // caller (hash_to_field) passes the 43-byte compile-time DST const.
    // Defense-in-depth debug_assert: catches a future misuse attempt in
    // tests without adding release-path CU.
    debug_assert!(dst.len() < 256, "RFC 9380 §5.3.3: DST must be < 256 bytes");

    // DST_prime = DST || I2OSP(len(DST), 1). Stack-allocated for T2.M.
    // Max 256 bytes: 255 DST + 1 length byte. Typical is 44 (43 + 1).
    let mut dst_prime = [0u8; 256];
    let dst_len = dst.len().min(255);
    dst_prime[..dst_len].copy_from_slice(&dst[..dst_len]);
    dst_prime[dst_len] = dst_len as u8;
    let dst_prime_slice = &dst_prime[..dst_len + 1];

    let z_pad = [0u8; 136]; // keccak256 rate (1088 bits = 136 bytes)

    // l_i_b_str = I2OSP(96, 2) = [0x00, 0x60]. Hardcoded — dropped the
    // `len_in_bytes` parameter and its `as u16` cast (CLIPPY-016).
    const L_I_B_STR: [u8; 2] = [0x00, 0x60];

    let zero_byte = [0u8; 1];

    // b_0 = keccak256(Z_pad || msg || l_i_b_str || I2OSP(0, 1) || DST_prime)
    let b_0 = keccak::hashv(&[&z_pad, msg, &L_I_B_STR, &zero_byte, dst_prime_slice]);

    // b_1 = keccak256(b_0 || I2OSP(1, 1) || DST_prime)
    let counter_1 = [1u8];
    let b_1 = keccak::hashv(&[b_0.as_ref(), &counter_1, dst_prime_slice]);

    // b_2 = keccak256(XOR(b_0, b_1) || I2OSP(2, 1) || DST_prime)
    let mut xor_buf = [0u8; 32];
    for (i, byte) in xor_buf.iter_mut().enumerate() {
        *byte = b_0.as_ref()[i] ^ b_1.as_ref()[i];
    }
    let counter_2 = [2u8];
    let b_2 = keccak::hashv(&[&xor_buf, &counter_2, dst_prime_slice]);

    // b_3 = keccak256(XOR(b_0, b_2) || I2OSP(3, 1) || DST_prime)
    for (i, byte) in xor_buf.iter_mut().enumerate() {
        *byte = b_0.as_ref()[i] ^ b_2.as_ref()[i];
    }
    let counter_3 = [3u8];
    let b_3 = keccak::hashv(&[&xor_buf, &counter_3, dst_prime_slice]);

    let mut result = [0u8; 96];
    result[0..32].copy_from_slice(b_1.as_ref());
    result[32..64].copy_from_slice(b_2.as_ref());
    result[64..96].copy_from_slice(b_3.as_ref());
    result
}

/// Convert 96 expanded bytes into two BN254 field elements.
/// Each from 48 bytes (384 bits > 254 bits), reduced mod p for uniform
/// distribution (RFC 9380 §5).
pub fn hash_to_field(msg: &[u8]) -> (Fq, Fq) {
    let expanded = expand_message_xmd(msg, DST);
    let u0 = Fq::from_be_bytes_mod_order(&expanded[0..48]);
    let u1 = Fq::from_be_bytes_mod_order(&expanded[48..96]);
    (u0, u1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_ff::BigInteger;

    fn fq_to_hex(fq: &Fq) -> String {
        let bigint = fq.into_bigint();
        let bytes = bigint.to_bytes_be();
        hex::encode(bytes)
    }

    #[test]
    fn expand_message_round_1() {
        let msg_hash = hex::decode("6c31fc15422ebad28aaf9089c306702f67540b53c7eea8b7d2941044b027100f").unwrap();
        let result = expand_message_xmd(&msg_hash, DST);
        let expected = hex::decode(
            "32fbeaeec0e8f16eb296583f44a5444067229f78974a4f8f1be5162c8966b110\
             4811f3b21495702b7d0ed5e137ee0bd1e9ba858a141f65a006d6d543c62a9c00\
             4d9d6d8da42a37613571828abc9095998c841d95db4bc6cc544bae10159ab061"
        ).unwrap();
        assert_eq!(result.to_vec(), expected, "Round 1 expand_message_xmd must match fixture");
    }

    #[test]
    fn expand_message_round_9337227() {
        let msg_hash = hex::decode("baf09720c37cb921fd8362b1d907232ac0b813ffba768c714aeaace987e7fd6b").unwrap();
        let result = expand_message_xmd(&msg_hash, DST);
        let expected = hex::decode(
            "c6ac26ea9c7aba18d279e0a442e24a4fc778321f5af60409b8cbb9ef64af1dd0\
             9ec8f85292c9d0b75a856229e501fb48d742778f14b2f4560e441a55868af2e9\
             9a6b7a85c2670598fb38a02ca749aeb981560fbc601b0345bebb4a5a68a0adc6"
        ).unwrap();
        assert_eq!(result.to_vec(), expected, "Round 9337227 expand_message_xmd must match fixture");
    }

    #[test]
    fn hash_to_field_round_1() {
        let msg_hash = hex::decode("6c31fc15422ebad28aaf9089c306702f67540b53c7eea8b7d2941044b027100f").unwrap();
        let (u0, u1) = hash_to_field(&msg_hash);
        assert_eq!(fq_to_hex(&u0), "1b163e041c11b8ddb908e7b705c98ca4f393243bf3664bf5934a3680d3a5bfc6");
        assert_eq!(fq_to_hex(&u1), "0b2f337436437aef114e4f8383ac665c24fe4d3f88b3c53d494ad4104b9d15eb");
    }

    #[test]
    fn hash_to_field_round_9337227() {
        let msg_hash = hex::decode("baf09720c37cb921fd8362b1d907232ac0b813ffba768c714aeaace987e7fd6b").unwrap();
        let (u0, u1) = hash_to_field(&msg_hash);
        assert_eq!(fq_to_hex(&u0), "109ead626603ce780c14be70861676828e42948357c960d53e4250cb47246064");
        assert_eq!(fq_to_hex(&u1), "1da61ba0e660ae1d421c04d6aa2a5d69b24a1a1d380d01b464bdf315b080e781");
    }
}
