//! Live devnet integration tests for `alea_sdk::cpi::verify`.
//!
//! Each test submits a real transaction to Solana devnet and asserts the
//! on-chain result matches the known drand fixture. All tests are gated
//! `#[ignore]` — they require:
//!   - A funded devnet wallet at ~/.config/solana/alea-deployer.json
//!   - Alea program deployed + Config PDA initialized on devnet (Phase 3 complete)
//!   - ~0.001 SOL per test (tx fees only — no rent, no data accounts)
//!
//! Run explicitly:
//!   cargo test -p alea-sdk --test devnet_verify -- --ignored
//!
//! Each test consumes ~0.001 SOL in devnet tx fees.

mod fixtures;

use alea_sdk::config_pda;
use fixtures::drand;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    signer::keypair::read_keypair_file,
    transaction::Transaction,
};
use std::collections::HashMap;
use std::str::FromStr;

const DEVNET_RPC: &str = "https://api.devnet.solana.com";
const CU_LIMIT: u32 = 900_000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_rpc() -> RpcClient {
    RpcClient::new_with_commitment(DEVNET_RPC.to_string(), CommitmentConfig::confirmed())
}

fn get_payer() -> Keypair {
    let wallet_path = format!(
        "{}/.config/solana/alea-deployer.json",
        std::env::var("HOME").expect("HOME must be set")
    );
    read_keypair_file(&wallet_path).unwrap_or_else(|_| panic!("keypair not found at {wallet_path}"))
}

/// Build the serialized data for a `verify(round, signature)` instruction.
///
/// Layout: 8-byte discriminator || round as little-endian u64 || signature as 64 raw bytes.
/// Anchor uses Borsh; u64 Borsh = 8-byte LE, [u8; 64] Borsh = 64 raw bytes (no length prefix
/// for fixed-size arrays).
fn build_verify_data(round: u64, signature: &[u8; 64]) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + 8 + 64);
    data.extend_from_slice(&drand::VERIFY_DISCRIMINATOR);
    data.extend_from_slice(&round.to_le_bytes());
    data.extend_from_slice(signature);
    data
}

/// Build a verify instruction targeting the live devnet Alea program.
fn build_verify_ix(payer: &Pubkey, round: u64, signature: &[u8; 64]) -> Instruction {
    let program_id =
        Pubkey::from_str(drand::PROGRAM_ID_STR).expect("PROGRAM_ID_STR must be a valid pubkey");
    let (cfg_pda, _bump) = config_pda(&program_id);
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(cfg_pda, false),
            AccountMeta::new_readonly(*payer, true),
        ],
        data: build_verify_data(round, signature),
    }
}

/// Minimal standard-alphabet base64 decode (no external dep).
fn base64_decode(s: &str) -> Vec<u8> {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let map: HashMap<u8, u8> = alphabet
        .bytes()
        .enumerate()
        .map(|(i, b)| (b, i as u8))
        .collect();
    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut i = 0;
    while i + 3 < bytes.len() {
        let a = *map.get(&bytes[i]).unwrap_or(&0) as u32;
        let b = *map.get(&bytes[i + 1]).unwrap_or(&0) as u32;
        let c = *map.get(&bytes[i + 2]).unwrap_or(&0) as u32;
        let d = *map.get(&bytes[i + 3]).unwrap_or(&0) as u32;
        let val = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((val >> 16) & 0xFF) as u8);
        out.push(((val >> 8) & 0xFF) as u8);
        out.push((val & 0xFF) as u8);
        i += 4;
    }
    if i + 2 < bytes.len() {
        let a = *map.get(&bytes[i]).unwrap_or(&0) as u32;
        let b = *map.get(&bytes[i + 1]).unwrap_or(&0) as u32;
        let c = *map.get(&bytes[i + 2]).unwrap_or(&0) as u32;
        let val = (a << 18) | (b << 12) | (c << 6);
        out.push(((val >> 16) & 0xFF) as u8);
        out.push(((val >> 8) & 0xFF) as u8);
    } else if i + 1 < bytes.len() {
        let a = *map.get(&bytes[i]).unwrap_or(&0) as u32;
        let b = *map.get(&bytes[i + 1]).unwrap_or(&0) as u32;
        let val = (a << 18) | (b << 12);
        out.push(((val >> 16) & 0xFF) as u8);
    }
    out
}

/// Submit a transaction with `skipPreflight = true`, then retry-poll for
/// the confirmed result. Retries up to 30s to account for Helius devnet
/// indexer lag (typically 2-5s per [[2026-04-18-helius-devnet-indexer-lag]]).
///
/// Returns (tx_sig, meta_err_string_or_none, return_data_bytes_or_none).
fn submit_and_get_meta(
    rpc: &RpcClient,
    tx: &Transaction,
) -> (String, Option<String>, Option<Vec<u8>>) {
    let config = solana_client::rpc_config::RpcSendTransactionConfig {
        skip_preflight: true,
        ..Default::default()
    };
    let sig = rpc
        .send_transaction_with_config(tx, config)
        .expect("send_transaction must succeed (network reachable)");
    let sig_str = sig.to_string();

    let confirmed_config = solana_client::rpc_config::RpcTransactionConfig {
        commitment: Some(CommitmentConfig::confirmed()),
        max_supported_transaction_version: Some(0),
        encoding: Some(solana_transaction_status::UiTransactionEncoding::Base64),
    };

    for _attempt in 0..15 {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Ok(confirmed) = rpc.get_transaction_with_config(&sig, confirmed_config) {
            let meta = confirmed
                .transaction
                .meta
                .as_ref()
                .expect("meta must be present");
            let meta_err = meta.err.as_ref().map(|e| format!("{e:?}"));
            let return_data = {
                use solana_transaction_status::option_serializer::OptionSerializer;
                match &meta.return_data {
                    OptionSerializer::Some(rd) => Some(base64_decode(&rd.data.0)),
                    _ => None,
                }
            };
            return (sig_str, meta_err, return_data);
        }
    }
    panic!("tx {sig_str} not confirmed within 30s on devnet");
}

/// Extract the `Custom` error code from a Solana InstructionError debug string.
/// Handles the format `InstructionError(N, Custom(M))` produced by `{:?}`.
fn extract_custom_error_code(meta_err: &str) -> Option<u32> {
    if let Some(idx) = meta_err.find("Custom(") {
        let rest = &meta_err[idx + 7..];
        if let Some(end) = rest.find(')') {
            if let Ok(code) = rest[..end].trim().parse::<u32>() {
                return Some(code);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
#[ignore = "hits devnet — run explicitly with -- --ignored; costs ~0.001 SOL"]
fn verify_round_1_fixture_against_devnet() {
    let rpc = get_rpc();
    let payer = get_payer();

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT);
    let verify_ix = build_verify_ix(&payer.pubkey(), drand::ROUND_1, &drand::ROUND_1_SIGNATURE);
    let recent_blockhash = rpc.get_latest_blockhash().expect("get_latest_blockhash");
    let tx = Transaction::new_signed_with_payer(
        &[cu_ix, verify_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let (sig_str, meta_err, return_data) = submit_and_get_meta(&rpc, &tx);

    assert!(
        meta_err.is_none(),
        "round 1 verify must succeed on devnet, got error: {meta_err:?}\ntx: {sig_str}"
    );

    let randomness = return_data.expect("round 1 verify must produce return data");
    assert_eq!(randomness.len(), 32, "return data must be exactly 32 bytes");
    assert_eq!(
        randomness.as_slice(),
        &drand::ROUND_1_EXPECTED_RANDOMNESS,
        "round 1 randomness must match expected sha256(sig) per ADR 0036\ntx: {sig_str}"
    );

    println!(
        "[devnet_verify] round 1 ok: tx={sig_str} randomness=0x{}",
        randomness.iter().fold(String::new(), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
    );
}

#[test]
#[ignore = "hits devnet — run explicitly with -- --ignored; costs ~0.001 SOL"]
fn verify_round_9337227_fixture() {
    let rpc = get_rpc();
    let payer = get_payer();

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT);
    let verify_ix = build_verify_ix(
        &payer.pubkey(),
        drand::ROUND_9337227,
        &drand::ROUND_9337227_SIGNATURE,
    );
    let recent_blockhash = rpc.get_latest_blockhash().expect("get_latest_blockhash");
    let tx = Transaction::new_signed_with_payer(
        &[cu_ix, verify_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let (sig_str, meta_err, return_data) = submit_and_get_meta(&rpc, &tx);

    assert!(
        meta_err.is_none(),
        "round 9337227 verify must succeed on devnet, got error: {meta_err:?}\ntx: {sig_str}"
    );

    let randomness = return_data.expect("round 9337227 verify must produce return data");
    assert_eq!(randomness.len(), 32, "return data must be exactly 32 bytes");
    assert_eq!(
        randomness.as_slice(),
        &drand::ROUND_9337227_EXPECTED_RANDOMNESS,
        "round 9337227 randomness must match expected sha256(sig) per ADR 0036\ntx: {sig_str}"
    );

    println!(
        "[devnet_verify] round 9337227 ok: tx={sig_str} randomness=0x{}",
        randomness.iter().fold(String::new(), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
    );
}

#[test]
#[ignore = "hits devnet — run explicitly with -- --ignored; costs ~0.001 SOL"]
fn wrong_round_fails_with_6000() {
    // Submit round=1 with the round-9337227 signature (on-curve but wrong pairing).
    // on_curve_g1 passes → pairing fails → AleaError::InvalidSignature (6000).
    let rpc = get_rpc();
    let payer = get_payer();

    let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(CU_LIMIT);
    let verify_ix = build_verify_ix(&payer.pubkey(), 1, &drand::ROUND_9337227_SIGNATURE);
    let recent_blockhash = rpc.get_latest_blockhash().expect("get_latest_blockhash");
    let tx = Transaction::new_signed_with_payer(
        &[cu_ix, verify_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let (sig_str, meta_err, _return_data) = submit_and_get_meta(&rpc, &tx);

    let err_str = meta_err
        .unwrap_or_else(|| panic!("wrong-round tx must fail on-chain; got success\ntx: {sig_str}"));

    let code = extract_custom_error_code(&err_str)
        .unwrap_or_else(|| panic!("expected Custom error code in: {err_str}\ntx: {sig_str}"));

    assert_eq!(
        code, 6000,
        "wrong-round sig must produce AleaError::InvalidSignature (6000), got {code}\ntx: {sig_str}"
    );

    println!("[devnet_verify] wrong_round_fails_with_6000 ok: code={code} tx={sig_str}");
}
