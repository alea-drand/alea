use anchor_lang::prelude::*;

use crate::crypto::constants::{
    EXPECTED_EVMNET_CHAIN_HASH, EXPECTED_EVMNET_GENESIS_TIME, EXPECTED_EVMNET_PERIOD,
    EXPECTED_EVMNET_PUBKEY,
};
use crate::errors::AleaError;
use crate::state::Config;

/// Accounts for the `initialize` instruction.
///
/// Creates the singleton Config PDA at seeds `["config"]` with the
/// deployer as authority. The `init` constraint returns Anchor's
/// built-in "account already in use" runtime error on duplicate
/// initialization (T3.22 — no custom error code needed).
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = authority,
        space = Config::LEN,
        seeds = [b"config"],
        bump,
    )]
    pub config: Account<'info, Config>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// `initialize` handler — sets up the Config PDA with evmnet parameters.
///
/// Guards (both compile-time constants, ADR 0031 + ADR 0027 fallback):
/// - `chain_hash == EXPECTED_EVMNET_CHAIN_HASH` (6007) — prevents
///   wrong-chain deployment (e.g., pointing Alea at a non-evmnet chain).
/// - `pubkey_g2 == EXPECTED_EVMNET_PUBKEY` (6008) — OPEN-ITEMS #4
///   RESOLVED 2026-04-16: fallback path chosen because the primary
///   `is_in_correct_subgroup_assuming_on_curve` exceeds 1.4M CU on BPF.
///   Key rotation requires a program upgrade.
pub fn initialize_handler(
    ctx: Context<Initialize>,
    pubkey_g2: [u8; 128],
    genesis_time: u64,
    period: u64,
    chain_hash: [u8; 32],
) -> Result<()> {
    require!(chain_hash == EXPECTED_EVMNET_CHAIN_HASH, AleaError::WrongChainHash);
    require!(pubkey_g2 == EXPECTED_EVMNET_PUBKEY, AleaError::WrongPubkey);
    // T2.E — byte-equality guards for all four Config fields. Prevents
    // genesis=0 / period=0 / wrong-constant attacks even if authority is
    // ever compromised. ADR 0031 extended.
    require!(genesis_time == EXPECTED_EVMNET_GENESIS_TIME, AleaError::InvalidGenesisTime);
    require!(period == EXPECTED_EVMNET_PERIOD, AleaError::InvalidPeriod);

    let config = &mut ctx.accounts.config;
    config.pubkey_g2 = pubkey_g2;
    config.genesis_time = genesis_time;
    config.period = period;
    config.chain_hash = chain_hash;
    config.authority = ctx.accounts.authority.key();
    config.bump = ctx.bumps.config;
    Ok(())
}
