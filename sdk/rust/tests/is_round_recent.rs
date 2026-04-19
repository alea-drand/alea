//! Unit tests for `is_round_recent`.
//!
//! Covers the three canonical cases:
//!   - Recent  (round emitted 5s ago)   → true
//!   - Stale   (round emitted 600s ago) → false
//!   - Boundary (round emitted exactly max_age_seconds ago) → true
//!
//! The live-Clock equivalent test against a real devnet Clock sysvar is
//! at `tests/devnet_clock.rs` and gated with `#[ignore]` so normal
//! `cargo test` does not hit the network.

use alea_sdk::{is_round_recent, Config};
use anchor_lang::prelude::Pubkey;
use anchor_lang::solana_program::sysvar::clock::Clock;

/// evmnet-realistic genesis + period used across all cases.
const GENESIS: u64 = 1_727_521_075;
const PERIOD: u64 = 3;
/// Canonical 30-second recency window (matches rust-cpi.md §"Palestra
/// Integration" MAX_BEACON_AGE_SECONDS default).
const MAX_AGE: u64 = 30;

fn config() -> Config {
    Config {
        pubkey_g2: [0u8; 128],
        genesis_time: GENESIS,
        period: PERIOD,
        chain_hash: [0u8; 32],
        authority: Pubkey::new_unique(),
        bump: 255,
    }
}

fn clock_at(unix_timestamp: i64) -> Clock {
    Clock {
        slot: 0,
        epoch_start_timestamp: 0,
        epoch: 0,
        leader_schedule_epoch: 0,
        unix_timestamp,
    }
}

/// Solve for the round whose emission timestamp is exactly `ts` seconds
/// after genesis: `ts = GENESIS + (round - 1) * PERIOD` → `round = (ts - GENESIS)/PERIOD + 1`.
fn round_emitted_at(ts: u64) -> u64 {
    ((ts - GENESIS) / PERIOD) + 1
}

#[test]
fn recent_round_5s_ago_returns_true() {
    // Current time = GENESIS + 1_000_000s (arbitrary far future; doesn't matter).
    // Round emitted 5s before that should be "recent" for max_age=30.
    let now: u64 = GENESIS + 1_000_000;
    let round = round_emitted_at(now - 5);
    let clock = clock_at(now as i64);
    assert!(
        is_round_recent(round, &config(), &clock, MAX_AGE),
        "round emitted 5s ago should be recent under 30s max_age"
    );
}

#[test]
fn stale_round_600s_ago_returns_false() {
    // 600s stale >> 30s max_age → reject.
    let now: u64 = GENESIS + 1_000_000;
    let round = round_emitted_at(now - 600);
    let clock = clock_at(now as i64);
    assert!(
        !is_round_recent(round, &config(), &clock, MAX_AGE),
        "round emitted 600s ago should be stale under 30s max_age"
    );
}

#[test]
fn boundary_round_exactly_max_age_ago_returns_true() {
    // Boundary case: age == max_age_seconds → comparison is `<=`, so accept.
    // Pick a round whose emission time is exactly MAX_AGE seconds before now.
    // To land on an exact round boundary, choose `now` such that
    // `(now - MAX_AGE - GENESIS)` is a multiple of PERIOD.
    // MAX_AGE=30, PERIOD=3, GENESIS even → pick now = GENESIS + 1_000_020 (divisible math).
    let now: u64 = GENESIS + 1_000_020;
    let emission_time = now - MAX_AGE;
    assert_eq!(
        (emission_time - GENESIS) % PERIOD,
        0,
        "test fixture invariant: emission time must land exactly on a round boundary"
    );
    let round = round_emitted_at(emission_time);
    let clock = clock_at(now as i64);
    assert!(
        is_round_recent(round, &config(), &clock, MAX_AGE),
        "round at exactly max_age_seconds age should be accepted (<= boundary is inclusive)"
    );
}

#[test]
fn just_past_boundary_returns_false() {
    // One second past boundary (age = MAX_AGE + 1) → reject.
    let now: u64 = GENESIS + 1_000_020;
    let emission_time = now - (MAX_AGE + 3); // use MAX_AGE + 3 to stay on round boundary
    let round = round_emitted_at(emission_time);
    let clock = clock_at(now as i64);
    assert!(
        !is_round_recent(round, &config(), &clock, MAX_AGE),
        "round 3s past max_age should be rejected"
    );
}

#[test]
fn zero_age_window_only_accepts_current_round() {
    // max_age = 0 is the tightest possible recency check.
    // Only a round emitted exactly at `now` passes.
    let now: u64 = GENESIS + 1_000_020;
    let round_now = round_emitted_at(now);
    let round_prev = round_now - 1;
    let clock = clock_at(now as i64);

    assert!(
        is_round_recent(round_now, &config(), &clock, 0),
        "current-slot round should pass age=0 check"
    );
    assert!(
        !is_round_recent(round_prev, &config(), &clock, 0),
        "previous round should fail age=0 check"
    );
}

#[test]
fn u64_max_round_saturates_not_overflows() {
    // Defensive: malformed input with round = u64::MAX must not panic.
    // `(round - 1) * period` would overflow without saturating_mul.
    let now: u64 = GENESIS + 1_000_020;
    let clock = clock_at(now as i64);
    // Saturating arithmetic yields round_timestamp = u64::MAX, so
    // saturating_sub(now, u64::MAX) = 0 ≤ max_age → returns true.
    // The important property is "doesn't panic" regardless of the bool.
    let _ = is_round_recent(u64::MAX, &config(), &clock, MAX_AGE);
}
