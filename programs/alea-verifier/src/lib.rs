#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U");

pub mod crypto;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

pub use instructions::initialize::*;
pub use instructions::update_config::*;
pub use instructions::verify::*;

// T1.04 — map_to_point_debug always re-exported (see instructions/mod.rs
// SECURITY POSTURE for why this is always-on and safe).
pub use instructions::map_to_point_debug::*;

#[program]
pub mod alea_verifier {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        pubkey_g2: [u8; 128],
        genesis_time: u64,
        period: u64,
        chain_hash: [u8; 32],
    ) -> Result<()> {
        initialize_handler(ctx, pubkey_g2, genesis_time, period, chain_hash)
    }

    pub fn verify(ctx: Context<Verify>, round: u64, signature: [u8; 64]) -> Result<[u8; 32]> {
        verify_handler(ctx, round, signature)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        pubkey_g2: [u8; 128],
        genesis_time: u64,
        period: u64,
        chain_hash: [u8; 32],
    ) -> Result<()> {
        update_config_handler(ctx, pubkey_g2, genesis_time, period, chain_hash)
    }

    // T1.04 — BPF-vs-native map_to_point parity debug instruction.
    // Always present in the shipped binary; stateless pure function with
    // zero attack surface. See `instructions/mod.rs` SECURITY POSTURE.
    pub fn map_to_point_debug(
        ctx: Context<MapToPointDebug>,
        u_bytes: [u8; 32],
    ) -> Result<[u8; 64]> {
        map_to_point_debug_handler(ctx, u_bytes)
    }
}
