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

    pub fn verify(
        ctx: Context<Verify>,
        round: u64,
        signature: [u8; 64],
    ) -> Result<[u8; 32]> {
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
}
