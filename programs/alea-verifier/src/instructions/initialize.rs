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
/// program's on-chain upgrade authority as authority. The `init`
/// constraint returns Anchor's built-in "account already in use"
/// runtime error on duplicate initialization (T3.22).
///
/// Wave X+1 / FENDER-002 hardening (Codex Session C, 2026-04-17):
/// `program_data` is the BPFLoaderUpgradeable ProgramData account
/// derived at `[crate::ID]` seeds in the loader program. It exposes
/// the program's `upgrade_authority_address` field. The handler
/// requires `authority.key() == program_data.upgrade_authority_address`
/// so only the entity that deployed the program (or holds the current
/// upgrade authority, e.g. Squads multisig post-transition per ADR 0009)
/// can initialize. This closes the deploy-to-init front-run window
/// physically at the program level; `scripts/initialize.ts` becomes
/// a convenience wrapper rather than the sole mitigation.
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
    /// ProgramData account for the Alea program. Anchor derives the PDA
    /// from `[crate::ID]` under the BPFLoaderUpgradeable program and
    /// deserializes the `upgrade_authority_address` field. Any caller
    /// whose `authority` pubkey does not match that field is rejected
    /// with `AleaError::UnauthorizedInit` (6012).
    #[account(
        seeds = [crate::ID.as_ref()],
        bump,
        seeds::program = anchor_lang::solana_program::bpf_loader_upgradeable::ID,
    )]
    pub program_data: Account<'info, ProgramData>,
    pub system_program: Program<'info, System>,
}

/// `initialize` handler — sets up the Config PDA with evmnet parameters.
///
/// Guards (ADR 0031 + ADR 0027 fallback + Wave X+1 FENDER-002 hardening):
/// - `authority == program_data.upgrade_authority_address` (6012) —
///   closes the deploy-to-init front-run window. Only the entity that
///   controls the program's upgrade authority (the deployer pre-transition,
///   the Squads 2-of-3 multisig post-transition per ADR 0009) can
///   initialize. Without this check, anyone watching the mempool between
///   `solana program deploy` confirmation and the initialize tx could
///   submit their own initialize with the correct (public) evmnet
///   constants and capture permanent authority (ADR 0009 forbids
///   rotation in `update_config`).
/// - `chain_hash == EXPECTED_EVMNET_CHAIN_HASH` (6007) — prevents
///   wrong-chain deployment.
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
    // Wave X+1 (Codex C HIGH, 2026-04-17) — upgrade-authority gate.
    // MUST be the first check so no state writes occur on unauthorized
    // calls. An immutable program (ADR 0009 Phase 5+) would have
    // `upgrade_authority_address == None`, in which case nobody can
    // initialize via this code path — that is the correct semantics
    // (immutable program's Config must be set before authority is
    // revoked).
    let upgrade_authority = ctx.accounts.program_data.upgrade_authority_address;
    require!(
        upgrade_authority == Some(ctx.accounts.authority.key()),
        AleaError::UnauthorizedInit,
    );

    require!(
        chain_hash == EXPECTED_EVMNET_CHAIN_HASH,
        AleaError::WrongChainHash
    );
    require!(pubkey_g2 == EXPECTED_EVMNET_PUBKEY, AleaError::WrongPubkey);
    // T2.E — byte-equality guards for all four Config fields. Prevents
    // genesis=0 / period=0 / wrong-constant attacks even if authority is
    // ever compromised. ADR 0031 extended.
    require!(
        genesis_time == EXPECTED_EVMNET_GENESIS_TIME,
        AleaError::InvalidGenesisTime
    );
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
