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

    #[test]
    fn config_len_is_217_bytes() {
        assert_eq!(Config::LEN, 217, "Config::LEN must equal spec.md §Account Schema (217 bytes)");
    }
}
