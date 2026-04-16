use anchor_lang::prelude::*;

/// Emitted on every successful `verify`.
///
/// `payer` is the tx signer that funded the verify call (RENAMED from
/// `verifier` per T2.27 — the program is the verifier). In user-pays
/// flows this exposes the end-user wallet in program logs; consumer
/// programs that need privacy sign via a PDA-derived signer instead.
///
/// Schema frozen per ADR 0028.
#[event]
pub struct BeaconVerified {
    pub round: u64,
    pub randomness: [u8; 32],
    pub payer: Pubkey,
}

/// Emitted on every successful `update_config`.
///
/// `pubkey_g2_hash` is the sha256 digest of `config.pubkey_g2` — NOT
/// the raw 128-byte pubkey (T3.m). Raw pubkey would bloat the event
/// log; subscribers who need the full bytes can decode the Config PDA
/// at the slot the event fired.
#[event]
pub struct ConfigUpdated {
    pub authority: Pubkey,
    pub chain_hash: [u8; 32],
    pub pubkey_g2_hash: [u8; 32],
}
