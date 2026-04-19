//! Account type re-exports for Alea CPI consumers.
//!
//! Consumer programs write their Accounts struct inline with the two
//! mandatory constraints per ADR 0034; see the lib.rs doc example for the
//! canonical pattern.

pub use alea_verifier::state::Config;
