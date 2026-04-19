//! Error type re-exports for Alea CPI consumers.
//!
//! `AleaError` variants map to on-chain error codes 6000-6012. See the
//! repository README §"Error Codes" for the full consumer-facing table
//! and retryability guidance.

pub use alea_verifier::errors::AleaError;
