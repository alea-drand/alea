//! Live-Clock test for `is_round_recent` against a real devnet Clock sysvar
//! + real initialized Config PDA. Phase 3.7 spec gate.
//!
//! Gated `#[ignore]` — normal `cargo test` does not hit the network. Run
//! with: `cargo test -p alea-sdk --test devnet_clock -- --ignored`
//!
//! Prerequisites:
//!   1. Alea program deployed to devnet (Phase D / D-bis complete).
//!   2. Config PDA initialized via `scripts/initialize.ts` (Phase E complete).

use alea_sdk::{config_pda, is_round_recent, Config, PROGRAM_ID};
use anchor_lang::AccountDeserialize;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, sysvar};

const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const MAX_AGE_SECONDS: u64 = 30;

/// Deserialize a Solana `Clock` sysvar from raw account data.
///
/// The Clock sysvar is bincode-encoded (not Borsh). Layout per
/// `solana_sdk::sysvar::clock::Clock`:
///   u64 slot | i64 epoch_start_timestamp | u64 epoch |
///   u64 leader_schedule_epoch | i64 unix_timestamp
fn deserialize_clock(data: &[u8]) -> solana_sdk::clock::Clock {
    bincode::deserialize(data).expect("Clock sysvar account data must bincode-deserialize")
}

/// Convert a `solana_sdk::clock::Clock` (what the RPC gives us) into the
/// `anchor_lang::solana_program::sysvar::clock::Clock` that
/// `is_round_recent` expects. Both types share the same memory layout.
fn clock_to_anchor(
    clock: solana_sdk::clock::Clock,
) -> anchor_lang::solana_program::sysvar::clock::Clock {
    anchor_lang::solana_program::sysvar::clock::Clock {
        slot: clock.slot,
        epoch_start_timestamp: clock.epoch_start_timestamp,
        epoch: clock.epoch,
        leader_schedule_epoch: clock.leader_schedule_epoch,
        unix_timestamp: clock.unix_timestamp,
    }
}

#[test]
#[ignore = "hits devnet — run explicitly with `-- --ignored`"]
fn is_round_recent_against_live_devnet_clock() {
    let rpc = RpcClient::new_with_commitment(DEVNET_RPC.to_string(), CommitmentConfig::confirmed());

    // 1. Fetch Clock sysvar.
    let clock_data = rpc
        .get_account_data(&sysvar::clock::ID)
        .expect("failed to fetch Clock sysvar from devnet");
    let clock_sdk = deserialize_clock(&clock_data);
    let clock = clock_to_anchor(clock_sdk.clone());
    println!(
        "[devnet_clock] Clock slot={} unix_timestamp={}",
        clock.slot, clock.unix_timestamp
    );

    // 2. Fetch Config PDA. Strip Anchor 8-byte discriminator, deserialize.
    let (pda, _bump) = config_pda(&PROGRAM_ID);
    println!("[devnet_clock] Config PDA: {}", pda);
    let config_data = rpc
        .get_account_data(&pda)
        .expect("failed to fetch Config PDA — is the program initialized on devnet?");
    assert_eq!(
        config_data.len(),
        Config::LEN,
        "Config PDA must be exactly 217 bytes"
    );
    let config = Config::try_deserialize(&mut config_data.as_ref())
        .expect("Config PDA data must Anchor-deserialize");
    println!(
        "[devnet_clock] Config genesis_time={} period={}",
        config.genesis_time, config.period
    );

    // 3. Compute what the current drand round should be on this slot.
    let current_timestamp = clock.unix_timestamp as u64;
    assert!(
        current_timestamp >= config.genesis_time,
        "devnet slot timestamp ({}) must be after evmnet genesis ({})",
        current_timestamp,
        config.genesis_time
    );
    let current_round = ((current_timestamp - config.genesis_time) / config.period) + 1;
    println!("[devnet_clock] computed current_round={}", current_round);

    // 4. Three assertions:
    //    (a) round 1 behind current → emitted ~period seconds ago → recent
    //    (b) round 200 behind current → emitted ~600s ago → stale
    //    (c) round (max_age / period) behind → emitted ~max_age_seconds ago → boundary (accept)

    // (a) Recent
    let recent = current_round - 1;
    assert!(
        is_round_recent(recent, &config, &clock, MAX_AGE_SECONDS),
        "round {} (~{}s ago) should be recent under {}s max_age",
        recent,
        config.period,
        MAX_AGE_SECONDS
    );

    // (b) Stale
    let stale = current_round.saturating_sub(200);
    assert!(
        !is_round_recent(stale, &config, &clock, MAX_AGE_SECONDS),
        "round {} (~{}s ago) should be stale under {}s max_age",
        stale,
        200 * config.period,
        MAX_AGE_SECONDS
    );

    // (c) Boundary — exactly max_age seconds ago
    let boundary_rounds_back = MAX_AGE_SECONDS / config.period;
    let boundary = current_round.saturating_sub(boundary_rounds_back);
    assert!(
        is_round_recent(boundary, &config, &clock, MAX_AGE_SECONDS),
        "round {} (~{}s ago) at the max_age boundary should be accepted (<= inclusive)",
        boundary,
        boundary_rounds_back * config.period
    );

    println!("[devnet_clock] ✓ all 3 live-Clock assertions passed");
}
