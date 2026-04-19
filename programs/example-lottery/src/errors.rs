use anchor_lang::prelude::*;

#[error_code]
pub enum GameError {
    /// Resolution round is before the committed minimum round.
    #[msg("Resolution round is before the committed minimum round")]
    RoundTooEarly,

    /// The drand beacon is too old — possible replay attack.
    #[msg("The drand beacon is too old (stale beacon)")]
    StaleBeacon,
}
