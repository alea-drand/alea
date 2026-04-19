//! Error type re-exports for Alea CPI consumers.
//!
//! `AleaError` variants map to on-chain error codes 6000-6009. See
//! `build-spec/sdk/rust-cpi.md` for the full table.

pub use alea_verifier::errors::AleaError;
