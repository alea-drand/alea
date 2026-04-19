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
        // Write all fields before taking any borrows for CPI.
        let player_key = ctx.accounts.player.key();
        let slot = ctx.accounts.clock.slot;
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
    pub fn resolve_bet(
        ctx: Context<ResolveBet>,
        round: u64,
        signature: [u8; 64],
    ) -> Result<()> {
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

        // Guard 3: CPI to Alea — one line.
        let randomness = alea_sdk::cpi::verify(
            ctx.accounts.alea_program.to_account_info(),
            ctx.accounts.alea_config.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            round,
            signature,
        )?;

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

        if player_wins {
            // Transfer locked SOL from Bet PDA back to player.
            **ctx
                .accounts
                .bet
                .to_account_info()
                .try_borrow_mut_lamports()? -= amount;
            **ctx.accounts.player.try_borrow_mut_lamports()? += amount;
            msg!(
                "resolve_bet: player {} won {} lamports",
                player_key,
                amount
            );
        } else {
            // Transfer locked SOL from Bet PDA to house (payer acts as house).
            **ctx
                .accounts
                .bet
                .to_account_info()
                .try_borrow_mut_lamports()? -= amount;
            **ctx.accounts.payer.try_borrow_mut_lamports()? += amount;
            msg!(
                "resolve_bet: house won {} lamports from {}",
                amount,
                player_key
            );
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
    /// multiple bets from the same player in the same epoch.
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

    /// The original player — receives SOL back if they win, or just rent on close.
    /// CHECK: validated via `address = bet.player` constraint.
    #[account(mut, address = bet.player)]
    pub player: UncheckedAccount<'info>,

    /// Transaction payer — acts as "house" if player loses.
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
