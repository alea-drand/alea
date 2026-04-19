//! Deterministic drand evmnet test vectors.
//!
//! Constants captured from api.drand.sh for the BN254 evmnet chain and
//! validated per ADR 0036 (randomness = sha256(signature)).
//!
//! These fixtures are used by:
//! - `devnet_verify.rs` for live devnet integration tests (gated `#[ignore]`)
//! - Any consumer wanting to cross-validate their own CPI integration
//!
//! Chain: drand evmnet BN254 unchained-on-g1
//! Chain hash: 04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3

#[allow(dead_code)]
pub mod drand {
    use hex_literal::hex;

    /// Alea program ID (canonical vanity per ADR 0028).
    pub const PROGRAM_ID_STR: &str = "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U";

    /// drand evmnet chain hash.
    pub const CHAIN_HASH: [u8; 32] =
        hex!("04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3");

    /// drand evmnet genesis timestamp (Unix seconds).
    pub const GENESIS_TIME: u64 = 1_727_521_075;

    /// drand evmnet round period (seconds).
    pub const PERIOD: u64 = 3;

    /// Verify instruction discriminator (from IDL — first 8 bytes of
    /// sha256("global:verify")).
    pub const VERIFY_DISCRIMINATOR: [u8; 8] = [133, 161, 141, 48, 120, 198, 88, 150];

    // -----------------------------------------------------------------------
    // Round 1 fixtures
    // -----------------------------------------------------------------------

    /// drand round number for the first fixture.
    pub const ROUND_1: u64 = 1;

    /// G1 signature for round 1 (64 bytes: x || y big-endian, uncompressed).
    /// Source: api.drand.sh evmnet chain, round 1, validated against on-chain
    /// pairing check (tests/alea.ts round-1 test vector).
    pub const ROUND_1_SIGNATURE: [u8; 64] = hex!(
        "11f812d738a36b2210dc88c2d635ad8039588205f42445d6de09e6530165c346"
        "2a23aca348c84badcf8df5321ac24577b7963d5b0d780bc4626baedb45cde373"
    );

    /// Expected 32-byte randomness for round 1 (sha256(signature) per ADR 0036).
    pub const ROUND_1_EXPECTED_RANDOMNESS: [u8; 32] =
        hex!("781b75698adc3af62cfa55db83cf0c73ae54e1ac8c0d4c3a2224126b65369ec5");

    // -----------------------------------------------------------------------
    // Round 9337227 fixtures
    // -----------------------------------------------------------------------

    /// drand round number for the second fixture.
    pub const ROUND_9337227: u64 = 9_337_227;

    /// G1 signature for round 9337227.
    pub const ROUND_9337227_SIGNATURE: [u8; 64] = hex!(
        "01d65d6128f4b2df3d08de85543d8efe06b0281d0770246ae3672e8ddd3efda0"
        "269373123458f0b5c0073eeed1c816a06809e127421513e34ee07df6987910b3"
    );

    /// Expected 32-byte randomness for round 9337227 (sha256(signature) per ADR 0036).
    pub const ROUND_9337227_EXPECTED_RANDOMNESS: [u8; 32] =
        hex!("a1e645cd6193837f626716851f5c42ad4bf63ad75193b2cae40f88c08c8f3bd8");
}
