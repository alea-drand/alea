use anchor_lang::prelude::*;

use crate::crypto::constants::{
    EXPECTED_EVMNET_CHAIN_HASH, EXPECTED_EVMNET_GENESIS_TIME, EXPECTED_EVMNET_PERIOD,
    EXPECTED_EVMNET_PUBKEY,
};
use crate::errors::AleaError;
use crate::events::ConfigUpdated;
use crate::state::Config;

/// Accounts for the `update_config` instruction.
///
/// `has_one = authority` is the authorization primitive — Anchor emits
/// `ConstraintHasOne` (error code 2001) automatically if the signer does
/// not match `config.authority`. NO custom `Unauthorized` variant exists
/// (T1.06 consolidation; see `program/spec.md §"Error Codes"`).
///
/// `bump = config.bump` uses the stored canonical bump instead of
/// re-deriving (≈10K CU saving per ADR 0028).
#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, Config>,
    pub authority: Signer<'info>,
}

/// `update_config` handler — same guards as `initialize`, different
/// authorization path (Anchor `has_one` fires before the handler body).
///
/// Does NOT modify `config.authority`. Authority rotation is out of
/// scope for this instruction to prevent accidental authority loss
/// through a typo; rotation happens via a separate SetAuthority flow
/// per ADR 0009.
///
/// Emits `ConfigUpdated { authority, chain_hash, pubkey_g2_hash }` where
/// `pubkey_g2_hash = sha256(config.pubkey_g2)` — a 32-byte digest rather
/// than the raw 128-byte pubkey to keep event logs small (T3.m).
pub fn update_config_handler(
    ctx: Context<UpdateConfig>,
    pubkey_g2: [u8; 128],
    genesis_time: u64,
    period: u64,
    chain_hash: [u8; 32],
) -> Result<()> {
    require!(
        chain_hash == EXPECTED_EVMNET_CHAIN_HASH,
        AleaError::WrongChainHash
    );
    require!(pubkey_g2 == EXPECTED_EVMNET_PUBKEY, AleaError::WrongPubkey);
    // T2.E — byte-equality guards for all four Config fields (symmetric
    // with initialize_handler). Prevents genesis=0 / period=0 / wrong-
    // constant attacks even if authority is ever compromised.
    require!(
        genesis_time == EXPECTED_EVMNET_GENESIS_TIME,
        AleaError::InvalidGenesisTime
    );
    require!(period == EXPECTED_EVMNET_PERIOD, AleaError::InvalidPeriod);

    let config = &mut ctx.accounts.config;

    // T2.D — idempotency guard. If all four fields match stored values,
    // early-return WITHOUT writing + WITHOUT emitting ConfigUpdated. This
    // eliminates the event-spam attack surface where a compromised or
    // buggy authority could spam indexers with no-op updates. Source:
    // P06-T2-01.
    if config.pubkey_g2 == pubkey_g2
        && config.genesis_time == genesis_time
        && config.period == period
        && config.chain_hash == chain_hash
    {
        return Ok(());
    }

    config.pubkey_g2 = pubkey_g2;
    config.genesis_time = genesis_time;
    config.period = period;
    config.chain_hash = chain_hash;
    // config.authority intentionally NOT modified (see doc-comment).

    // T2.H — NOTE: pubkey_g2_hash reflects the POST-write value (hashes
    // the just-stored config.pubkey_g2). Consumer indexers that want to
    // detect rotation (old → new) must track state externally or query
    // the previous slot's Config. Schema frozen per ADR 0028; adding a
    // prev_hash field would require an ADR amendment.
    let pubkey_g2_hash = anchor_lang::solana_program::hash::hash(&config.pubkey_g2).to_bytes();
    emit!(ConfigUpdated {
        authority: config.authority,
        chain_hash: config.chain_hash,
        pubkey_g2_hash,
    });
    Ok(())
}
