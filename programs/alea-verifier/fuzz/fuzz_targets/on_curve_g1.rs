#![no_main]

// Fuzz target: on_curve_g1 (CVE-2025-30147 regression target)
//
// Goal: find any 64-byte input where Alea's on_curve_g1 disagrees with an
// INDEPENDENT arkworks-based check.
//
// The CVE-2025-30147 bug class (Besu, May 2025) is: subgroup check is
// performed BEFORE on-curve check, so a point that IS in the target subgroup
// but NOT on the curve silently passes.
//
// Alea's `on_curve_g1` does:
//   1. Check x < p AND y < p (canonical form)
//   2. Check y² == x³ + 3 mod p (curve equation)
//
// For BN254, G1 has cofactor 1, so subgroup == full curve. No separate
// subgroup check is needed. But if anyone introduces one in the future AND
// re-orders it before on_curve, the bug regresses.
//
// This fuzzer builds an independent arkworks-based point validator and asserts
// it agrees with Alea's on_curve_g1 byte-for-byte on every input.

use alea_verifier::crypto::pairing::on_curve_g1;
use ark_bn254::{Fq, G1Affine};
use ark_ec::AffineRepr;
use ark_ff::{BigInteger, PrimeField};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: [u8; 64]| {
    let alea_result = on_curve_g1(&bytes);

    // Independent reference: parse x, y using arkworks, check curve equation directly.
    let x_bytes: [u8; 32] = bytes[..32].try_into().unwrap();
    let y_bytes: [u8; 32] = bytes[32..].try_into().unwrap();

    // Canonical form check: x < p AND y < p.
    // We use arkworks' PrimeField::from_be_bytes_mod_order to convert, but then
    // verify the reverse conversion matches. If reduction happened, x or y was
    // non-canonical.
    let x = Fq::from_be_bytes_mod_order(&x_bytes);
    let y = Fq::from_be_bytes_mod_order(&y_bytes);

    // Re-encode to compare: if byte-identical to input, value was canonical.
    let x_reenc = x.into_bigint().to_bytes_be();
    let y_reenc = y.into_bigint().to_bytes_be();

    let x_canonical = x_reenc.len() == 32 && x_reenc == x_bytes;
    let y_canonical = y_reenc.len() == 32 && y_reenc == y_bytes;

    if !x_canonical || !y_canonical {
        // Input non-canonical; Alea must reject.
        assert!(
            !alea_result,
            "Alea accepted non-canonical point: x_canonical={}, y_canonical={}, bytes={:?}",
            x_canonical, y_canonical, hex::encode(&bytes[..])
        );
        return;
    }

    // Canonical: check curve equation.
    let point = G1Affine::new_unchecked(x, y);
    let ref_result = point.is_on_curve() && !point.is_zero();

    // Alea and reference must agree.
    assert_eq!(
        alea_result, ref_result,
        "on_curve_g1 disagreement: alea={}, ref={}, bytes={:?}",
        alea_result, ref_result, hex::encode(&bytes[..])
    );
});
