use anchor_lang::prelude::*;

#[error_code]
pub enum GameError {
    /// Resolution round is before the committed minimum round.
    #[msg("Resolution round is before the committed minimum round")]
    RoundTooEarly,

    /// The drand beacon is too old — possible replay attack.
    #[msg("The drand beacon is too old (stale beacon)")]
    StaleBeacon,

    /// Bet amount must be greater than zero.
    #[msg("Bet amount must be greater than zero")]
    ZeroAmount,

    /// min_resolution_round is in the past — it must be a future drand round
    /// that the player cannot have observed at commit time.
    #[msg("min_resolution_round must be a future drand round (anti-front-run)")]
    MinRoundInPast,

    /// Bet PDA lamport balance is insufficient to pay out — indicates either
    /// a griefing attempt (someone drained funds outside the normal flow) or
    /// a state-corruption bug. Hard-fail with checked arithmetic.
    #[msg("Bet PDA lamport balance is insufficient to pay out")]
    InsufficientFunds,

    /// Arithmetic overflow when crediting the payout destination. Vanishingly
    /// unlikely (would require the house/player wallet to hold ~u64::MAX
    /// lamports already) but checked to be defensive.
    #[msg("Payout overflow")]
    PayoutOverflow,
}
