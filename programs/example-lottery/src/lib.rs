//! Example Lottery — canonical Alea CPI consumer demonstrating commit-reveal pattern.
//!
//! This program shows how to integrate alea-sdk correctly with all mandatory
//! security constraints:
//!   1. `seeds::program = alea_program.key()` on the Alea config account (ADR 0034)
//!   2. `is_round_recent()` check before trusting randomness
//!   3. Immediate capture of return data before any other CPIs
//!   4. Commit-reveal to prevent front-running (T2.HH — SHOULD for high-stakes)
//!
//! This is a test-only / demonstration program. `publish = false` in Cargo.toml.
//! Use `anchor build --no-idl -p example-lottery` for local testing.
//!
//! # Correlated-randomness warning (Phase 4.5 Marcus T2)
//!
//! Multiple bets resolving against the SAME drand round share the same 32
//! bytes of randomness — each bet merely samples a different u64 window
//! (e.g., bytes[0..8] vs bytes[8..16]). For a 50/50 lottery this is
//! economically neutral (both outcomes are still 50/50 per-bet). For
//! asymmetric games (house-edge lotteries, tournaments with payout tiers)
//! players can COORDINATE their commits so all resolve on the same round,
//! effectively playing against each other instead of against the house.
//!
//! Mitigations for asymmetric consumers:
//! - Require each resolution to use a UNIQUE round (track used_rounds
//!   in a PDA set)
//! - Or bucket bets by ticket count and force rounds apart by commit-slot
//!   spacing such that two bets cannot resolve on the same round
//! - Or derive per-bet randomness = sha256(round_randomness || bet_pda_key)

// Suppress Anchor 0.30.1's harmless `anchor-debug` cfg warning (same as
// programs/alea-verifier/src/lib.rs).
#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;
use anchor_lang::system_program;

mod errors;
pub use errors::GameError;

declare_id!("ExLotTerY1111111111111111111111111111111111");

/// How many seconds old a drand beacon can be before we reject it.
const MAX_BEACON_AGE_SECONDS: u64 = 30;

// ---------------------------------------------------------------------------
// Program instructions
// ---------------------------------------------------------------------------

#[program]
pub mod example_lottery {
    use super::*;

    /// Commit a bet. Initializes a `Bet` PDA, locks SOL, and records the
    /// minimum drand round that can be used to resolve it. This prevents
    /// front-running: the resolver must use a round emitted AFTER the commit.
    pub fn commit_bet(
        ctx: Context<CommitBet>,
        amount: u64,
        min_resolution_round: u64,
    ) -> Result<()> {
        // Guard: reject zero-amount bets (otherwise the Bet PDA's locked
        // lamports equal its rent exemption and the resolve_bet lamport
        // math produces a no-op payout).
        require!(amount > 0, GameError::ZeroAmount);

        // Guard: min_resolution_round must be a FUTURE drand round relative
        // to the current slot. Without this floor, a player could pass
        // min_resolution_round = 0 and self-resolve using a round they
        // already observed, defeating commit-reveal's anti-front-run property.
        let alea_config = &ctx.accounts.alea_config;
        let clock = &ctx.accounts.clock;
        // Phase 4.5 T2-01 symmetry with alea_sdk: clamp negative i64 before cast.
        let current_ts = clock.unix_timestamp.max(0) as u64;
        // Anti-front-run floor: pick the smallest round whose emission time is
        // strictly AFTER current_ts, so the player cannot have observed its
        // randomness at commit time.
        //
        // emission_time(R) = genesis + (R-1) * period.
        // We want R such that emission_time(R) > current_ts, i.e.
        //     R > (current_ts - genesis) / period + 1
        //
        // In integer math this simplifies to:
        //     R >= floor((current_ts - genesis) / period) + 2
        //
        // Equivalently (used here): add `period` to current_ts, floor-divide,
        // and add 1. When current_ts sits exactly on a period boundary the two
        // formulas agree; when it's mid-period they still agree because the
        // extra `period` bumps the floor-div exactly when the boundary crosses.
        let min_allowed_ts = current_ts.saturating_add(alea_config.period);
        let min_allowed_round = min_allowed_ts
            .saturating_sub(alea_config.genesis_time)
            .saturating_div(alea_config.period)
            .saturating_add(1);
        require!(
            min_resolution_round >= min_allowed_round,
            GameError::MinRoundInPast
        );

        // Write all fields before taking any borrows for CPI.
        let player_key = ctx.accounts.player.key();
        let slot = clock.slot;
        let bump = ctx.bumps.bet;
        {
            let bet = &mut ctx.accounts.bet;
            bet.player = player_key;
            bet.amount = amount;
            bet.min_resolution_round = min_resolution_round;
            bet.slot = slot;
            bet.bump = bump;
        }

        // Transfer SOL from player to the Bet PDA to lock it.
        let cpi_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.player.to_account_info(),
                to: ctx.accounts.bet.to_account_info(),
            },
        );
        system_program::transfer(cpi_ctx, amount)?;

        msg!(
            "commit_bet: player={} amount={} min_round={}",
            player_key,
            amount,
            min_resolution_round
        );
        Ok(())
    }

    /// Resolve a bet using a verified drand beacon.
    ///
    /// Guard order is load-bearing — do not reorder:
    ///   1. round >= min_resolution_round (prevent pre-commit replay)
    ///   2. is_round_recent (prevent stale-beacon replay)
    ///   3. alea_sdk::cpi::verify (on-chain BLS verification via CPI)
    ///   4. Capture randomness IMMEDIATELY (return-data ordering invariant)
    ///   5. Determine winner + pay out
    pub fn resolve_bet(ctx: Context<ResolveBet>, round: u64, signature: [u8; 64]) -> Result<()> {
        let bet = &ctx.accounts.bet;

        // Guard 1: round must be at least the minimum committed round.
        require!(round >= bet.min_resolution_round, GameError::RoundTooEarly);

        // Guard 2: reject stale beacons (replays of old known-randomness rounds).
        require!(
            alea_sdk::is_round_recent(
                round,
                &ctx.accounts.alea_config,
                &ctx.accounts.clock,
                MAX_BEACON_AGE_SECONDS,
            ),
            GameError::StaleBeacon
        );

        // Guard 3: CPI to Alea — one line. Returns VerifiedRandomness
        // (must_use wrapper so a forgotten `.into_inner()` / `.as_bytes()`
        // produces a compile warning instead of silently dropping bytes).
        let randomness = alea_sdk::cpi::verify(
            ctx.accounts.alea_program.to_account_info(),
            ctx.accounts.alea_config.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            round,
            signature,
        )?
        .into_inner();

        // Guard 4: capture return data IMMEDIATELY — Solana overwrites on next CPI.
        // `randomness` is already a [u8; 32] local variable above.
        // The SOL transfers below happen AFTER the capture.

        let random_value = u64::from_le_bytes(randomness[0..8].try_into().unwrap());
        let player_wins = random_value % 2 == 0;

        let amount = bet.amount;
        let player_key = bet.player;

        msg!(
            "resolve_bet: round={} random_value={} player_wins={}",
            round,
            random_value,
            player_wins
        );

        // Phase 4.5 T1-16 rewrite: use checked arithmetic to eliminate the
        // underflow footgun previous direct `-=` had, and keep the payout
        // logic safe regardless of griefing.
        //
        // Semantics by outcome:
        //   - player_wins: do nothing manual here. `close = player` at
        //     instruction end moves the PDA's remaining lamports (rent +
        //     locked amount) to `player`. Net to player: +amount.
        //   - player_loses: explicitly transfer `amount` from Bet PDA to
        //     payer via checked math. `close = player` then moves the
        //     remaining rent-exempt balance back to player (refund).
        //     Net to payer: +amount; net to player: 0 (rent in, rent out).
        //
        // Checked math converts what was previously a latent panic on
        // underflow into a clean GameError that a consumer can handle.
        if !player_wins {
            let bet_info = ctx.accounts.bet.to_account_info();
            let current_bet_lamports = bet_info.lamports();
            let new_bet_lamports = current_bet_lamports
                .checked_sub(amount)
                .ok_or(GameError::InsufficientFunds)?;
            let payer_info = ctx.accounts.payer.to_account_info();
            let new_payer_lamports = payer_info
                .lamports()
                .checked_add(amount)
                .ok_or(GameError::PayoutOverflow)?;

            **bet_info.try_borrow_mut_lamports()? = new_bet_lamports;
            **payer_info.try_borrow_mut_lamports()? = new_payer_lamports;

            msg!(
                "resolve_bet: house won {} lamports from {}",
                amount,
                player_key
            );
        } else {
            // Winning path: close = player constraint handles the payout.
            msg!("resolve_bet: player {} won {} lamports", player_key, amount);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(amount: u64, min_resolution_round: u64)]
pub struct CommitBet<'info> {
    /// The Bet PDA — initialized here, holds the locked SOL + bet metadata.
    /// Seeds: [b"bet", player.key(), slot.to_le_bytes()] — slot disambiguates
    /// multiple bets from the same player in the same slot.
    #[account(
        init,
        payer = player,
        space = Bet::LEN,
        seeds = [b"bet", player.key().as_ref(), &clock.slot.to_le_bytes()],
        bump,
    )]
    pub bet: Account<'info, Bet>,

    #[account(mut)]
    pub player: Signer<'info>,

    /// Alea program — needed only so the alea_config seeds::program constraint
    /// works for the commit-time future-round check. Not invoked via CPI here.
    pub alea_program: Program<'info, alea_sdk::AleaVerifier>,

    /// Alea Config PDA (read-only). Used to compute the current drand round at
    /// commit time so we can enforce min_resolution_round >= current_round + 1.
    #[account(
        seeds = [b"config"],
        bump = alea_config.bump,
        seeds::program = alea_program.key(),
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,

    pub clock: Sysvar<'info, Clock>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolveBet<'info> {
    /// The Bet PDA being resolved.
    #[account(
        mut,
        seeds = [b"bet", bet.player.as_ref(), &bet.slot.to_le_bytes()],
        bump = bet.bump,
        close = player,
    )]
    pub bet: Account<'info, Bet>,

    /// The original player. Required as Signer — only the player can resolve
    /// their own bet. This prevents a griefer from force-resolving at a
    /// preferred drand round to bias the outcome. (The spec's Palestra example
    /// uses a permissionless-resolve pattern because Palestra has higher-level
    /// game state that prevents round-selection attacks; a bare lottery does
    /// not, so we require player signature here.)
    #[account(mut, address = bet.player)]
    pub player: Signer<'info>,

    /// Transaction payer — acts as "house" if player loses. Typically the
    /// same as player for self-play, or a dedicated house wallet for custodial
    /// lotteries.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Alea program for randomness verification.
    pub alea_program: Program<'info, alea_sdk::AleaVerifier>,

    /// Alea Config PDA. MUST include seeds::program per ADR 0034.
    #[account(
        seeds = [b"config"],
        bump = alea_config.bump,
        seeds::program = alea_program.key(),   // ← MANDATORY. Do not remove.
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,

    /// Clock sysvar for is_round_recent() check.
    pub clock: Sysvar<'info, Clock>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[account]
pub struct Bet {
    /// The player who placed this bet.
    pub player: Pubkey,
    /// Locked SOL amount (lamports).
    pub amount: u64,
    /// Minimum drand round that can be used to resolve. Prevents using a round
    /// the player knew at commit time.
    pub min_resolution_round: u64,
    /// Slot at commit time — part of the PDA seed for multiple-bet support.
    pub slot: u64,
    /// Canonical bump for this PDA.
    pub bump: u8,
}

impl Bet {
    /// 8 discriminator + 32 player + 8 amount + 8 min_resolution_round + 8 slot + 1 bump
    pub const LEN: usize = 8 + 32 + 8 + 8 + 8 + 1;
}
