use anchor_lang::prelude::*;

/// drand evmnet configuration account.
///
/// Single PDA at `["config"]`. Stores the evmnet chain parameters
/// needed to verify BLS signatures on-chain. Schema is part of the
/// v1 CPI interface and frozen per ADR 0028 — exact field order and
/// sizes must not change across program upgrades.
#[account]
pub struct Config {
    /// drand evmnet G2 public key (uncompressed, Kyber byte ordering:
    /// `x_c1 || x_c0 || y_c1 || y_c0`, each 32 BE bytes).
    pub pubkey_g2: [u8; 128],

    /// Genesis timestamp of the evmnet chain (Unix seconds).
    pub genesis_time: u64,

    /// Round period in seconds.
    pub period: u64,

    /// evmnet chain hash (identifies which drand chain this config points at).
    pub chain_hash: [u8; 32],

    /// Authority that can call `update_config`. `has_one` in the
    /// `UpdateConfig` Accounts struct enforces the match.
    pub authority: Pubkey,

    /// Canonical PDA bump stored at init time. `verify` and
    /// `update_config` use `bump = config.bump` to skip re-derivation
    /// (~10K CU saving).
    pub bump: u8,
}

impl Config {
    /// Exact account size: 8 (Anchor discriminator) + 128 + 8 + 8 + 32
    /// + 32 + 1 = 217 bytes. Per `program/spec.md §"Account Schema"`.
    pub const LEN: usize = 8 + 128 + 8 + 8 + 32 + 32 + 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{AnchorDeserialize, AnchorSerialize};

    #[test]
    fn config_len_is_217_bytes() {
        assert_eq!(Config::LEN, 217, "Config::LEN must equal spec.md §Account Schema (217 bytes)");
    }

    // P08-T3-03 (Phase 2.5 Wave I, Bucket A) — Borsh serialization round-trip.
    // Schema is frozen per ADR 0028; this test pins the exact field layout
    // (order, sizes) and byte length so any accidental reordering or type
    // change surfaces immediately at `cargo test`.
    #[test]
    fn config_borsh_roundtrip_pins_v1_schema() {
        let original = Config {
            pubkey_g2: [0xAB; 128],
            genesis_time: 0x1122_3344_5566_7788,
            period: 3,
            chain_hash: [0xCD; 32],
            authority: Pubkey::new_unique(),
            bump: 255,
        };

        let bytes = original.try_to_vec().expect("serialize must succeed");
        // Expected payload = 128 + 8 + 8 + 32 + 32 + 1 = 209 bytes (Config::LEN
        // minus the 8-byte Anchor discriminator that prefixes on-chain accounts
        // but not the raw Borsh-serialized struct).
        assert_eq!(
            bytes.len(),
            Config::LEN - 8,
            "Borsh-serialized Config must be exactly 209 bytes (Config::LEN - 8 discriminator)",
        );

        // Byte-layout sanity: Borsh writes fields in declaration order, so the
        // first 128 bytes MUST equal pubkey_g2, the next 8 bytes MUST be
        // genesis_time (little-endian u64), etc.
        assert_eq!(&bytes[0..128], &[0xAB; 128], "pubkey_g2 must be first field");
        assert_eq!(&bytes[128..136], &0x1122_3344_5566_7788u64.to_le_bytes(),
            "genesis_time must follow pubkey_g2 as LE u64");
        assert_eq!(&bytes[136..144], &3u64.to_le_bytes(),
            "period must follow genesis_time as LE u64");
        assert_eq!(&bytes[144..176], &[0xCD; 32],
            "chain_hash must follow period");
        // authority (Pubkey) at 176..208; bump at 208.
        assert_eq!(bytes[208], 255, "bump must be the last byte");

        let recovered = Config::try_from_slice(&bytes).expect("deserialize must succeed");
        assert_eq!(recovered.pubkey_g2, original.pubkey_g2);
        assert_eq!(recovered.genesis_time, original.genesis_time);
        assert_eq!(recovered.period, original.period);
        assert_eq!(recovered.chain_hash, original.chain_hash);
        assert_eq!(recovered.authority, original.authority);
        assert_eq!(recovered.bump, original.bump);
    }
}
