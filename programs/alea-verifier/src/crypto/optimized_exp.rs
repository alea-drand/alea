// Addition chain for x^((p+1)/4) on BN254 Fq.
// Source: gnark-crypto ecc/bn254/fp/element_exp.go
// 246 squares + 54 multiplies = 300 total ops
//
// BPF stack: 4KB per frame, 32KB total (8 frames max).
// Strategy: hold precomp + accumulator in ONE frame (~800 bytes),
// delegate multiply/square to #[inline(never)] wrappers so ark-ff
// internals get their own frames without blowing our budget.
use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, Field};

#[inline(never)]
fn fq_mul(a: &Fq, b: &Fq) -> Fq {
    *a * *b
}

#[inline(never)]
fn fq_sq(a: &Fq) -> Fq {
    a.square()
}

#[inline(never)]
fn fq_sqn(x: Fq, n: u32) -> Fq {
    let mut r = x;
    for _ in 0..n {
        r = fq_sq(&r);
    }
    r
}

#[inline(never)]
pub fn sqrt_and_check(x: &Fq) -> Option<Fq> {
    if *x == Fq::ZERO {
        return Some(Fq::ZERO);
    }
    let s = sqrt_chain(x);
    if fq_mul(&s, &s) == *x {
        Some(s)
    } else {
        None
    }
}

#[inline(never)]
fn sqrt_chain(x: &Fq) -> Fq {
    // Precompute 22 values (704 bytes on stack)
    let p0 = fq_sq(x);                 // _10
    let p1 = fq_mul(x, &p0);           // _11
    let p2 = fq_mul(&p0, &p1);         // _101
    let p3 = fq_mul(x, &p2);           // _110
    let p4 = fq_mul(x, &p3);           // _111
    let p5 = fq_mul(&p2, &p3);         // _1011
    let p6 = fq_mul(x, &p5);           // _1100
    let p7 = fq_mul(x, &p6);           // _1101
    let p8 = fq_mul(&p0, &p7);         // _1111
    let p9 = fq_mul(&p0, &p8);         // _10001
    let p10 = fq_mul(&p0, &p9);        // _10011
    let p11 = fq_mul(&p3, &p9);        // _10111
    let p12 = fq_mul(&p0, &p11);       // _11001
    let p13 = fq_mul(&p0, &p12);       // _11011
    let p14 = fq_mul(&p3, &p12);       // _11111
    let p15 = fq_mul(&p6, &p11);       // _100011
    let p16 = fq_mul(&p6, &p13);       // _100111
    let p17 = fq_mul(&p0, &p16);       // _101001
    let p18 = fq_mul(&p0, &p17);       // _101011
    let p19 = fq_mul(&p0, &p18);       // _101101
    let p20 = fq_mul(&p6, &p19);       // _111001
    let p21 = fq_mul(&p16, &p20);      // _1100000

    // Run chain using the non-inlined wrappers
    chain_part1(x, &p0, &p1, &p2, &p3, &p4, &p5, &p7, &p8, &p9, &p10,
                &p11, &p12, &p13, &p14, &p15, &p16, &p17, &p18, &p19, &p20, &p21)
}

#[inline(never)]
fn chain_part1(
    x: &Fq, _p0: &Fq, p1: &Fq, p2: &Fq, _p3: &Fq, p4: &Fq, _p5: &Fq,
    p7: &Fq, p8: &Fq, p9: &Fq, p10: &Fq, p11: &Fq, p12: &Fq, p13: &Fq,
    _p14: &Fq, _p15: &Fq, p16: &Fq, p17: &Fq, p18: &Fq, _p19: &Fq,
    p20: &Fq, p21: &Fq,
) -> Fq {
    let mut t = fq_sqn(*p21, 5); t = fq_mul(&t, p12);
    t = fq_sqn(t, 9); t = fq_mul(&t, p16);
    t = fq_sqn(t, 8); t = fq_mul(&t, p20);
    t = fq_sqn(t, 4); t = fq_mul(&t, p4);
    t = fq_sqn(t, 9); t = fq_mul(&t, p10);
    t = fq_sqn(t, 7); t = fq_mul(&t, p7);
    t = fq_sqn(t, 13); t = fq_mul(&t, p17);
    t = fq_sqn(t, 5); t = fq_mul(&t, p11);
    t = fq_sqn(t, 7); t = fq_mul(&t, p2);
    t = fq_sqn(t, 10); t = fq_mul(&t, p9);
    t = fq_sqn(t, 6); t = fq_mul(&t, p13);
    t = fq_sqn(t, 5); t = fq_mul(&t, p7);
    t = fq_sqn(t, 8); t = fq_mul(&t, p1);
    t = fq_sqn(t, 12); t = fq_mul(&t, p18);
    t = fq_sqn(t, 9); t = fq_mul(&t, p11);
    t = fq_sqn(t, 6); t = fq_mul(&t, p12);
    t = fq_sqn(t, 5); t = fq_mul(&t, p8);
    chain_part2(t, x, _p0, p2, _p3, _p5, p4, p7, p8, _p14, _p15, p17, _p19, p20)
}

#[inline(never)]
fn chain_part2(
    mut t: Fq, x: &Fq, _p0: &Fq, p2: &Fq, _p3: &Fq, p5: &Fq,
    p4: &Fq, p7: &Fq, p8: &Fq, p14: &Fq, p15: &Fq, p17: &Fq,
    p19: &Fq, p20: &Fq,
) -> Fq {
    t = fq_sqn(t, 12); t = fq_mul(&t, p19);
    t = fq_sqn(t, 7); t = fq_mul(&t, p17);
    t = fq_sqn(t, 9); t = fq_mul(&t, p19);
    t = fq_sqn(t, 7); t = fq_mul(&t, p4);
    t = fq_sqn(t, 9); t = fq_mul(&t, p20);
    t = fq_sqn(t, 4); t = fq_mul(&t, p2);
    t = fq_sqn(t, 7); t = fq_mul(&t, p7);
    t = fq_sqn(t, 6); t = fq_mul(&t, p8);
    t = fq_sqn(t, 5); t = fq_mul(&t, x);
    t = fq_sqn(t, 11); t = fq_mul(&t, p15);
    t = fq_sqn(t, 11); t = fq_mul(&t, p19);
    t = fq_sqn(t, 4); t = fq_mul(&t, p5);
    t = fq_sqn(t, 9); t = fq_mul(&t, p14);
    t = fq_sqn(t, 8); t = fq_mul(&t, p20); t = fq_mul(&t, _p3);
    t = fq_sqn(t, 7); t = fq_mul(&t, p17);
    fq_sq(&t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_sqrt_matches_ark() {
        for val in [1u64, 2, 3, 4, 7, 9, 16, 42, 100, 12345678] {
            let x = Fq::from(val);
            let ark_result = x.sqrt();
            let chain_result = sqrt_and_check(&x);
            match (ark_result, chain_result) {
                (Some(a), Some(b)) => {
                    assert!(a == b || a == -b, "sqrt mismatch for {val}");
                }
                (None, None) => {}
                _ => panic!("sqrt disagreement for {val}"),
            }
        }
    }

    #[test]
    fn chain_sqrt_edge_cases() {
        assert_eq!(sqrt_and_check(&Fq::ZERO), Some(Fq::ZERO));
        let one = Fq::from(1u64);
        let s = sqrt_and_check(&one).unwrap();
        assert!(s == one || s == -one);
    }
}
