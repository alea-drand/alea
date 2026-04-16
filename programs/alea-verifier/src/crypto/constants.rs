use ark_bn254::Fq;
use ark_ff::{AdditiveGroup, Field};

pub const BN254_B: u64 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fq_basic_ops() {
        let x = Fq::from(42u64);
        let y = x.square();
        assert_ne!(y, Fq::ZERO);
        assert_eq!(x * x, y);
    }
}
