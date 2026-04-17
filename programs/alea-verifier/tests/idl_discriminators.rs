//! T2.A — IDL discriminator verification test.
//!
//! Because Alea ships a hand-written IDL (Anchor 0.30.1 + Rust 1.94.1
//! proc-macro2 incompat prevents auto-generation via `anchor build`),
//! there is no compile-time guarantee that `target/idl/alea_verifier.json`
//! discriminators match Anchor's `sha256("<kind>:<name>")[..8]` convention.
//! A single wrong byte in any discriminator causes `InstructionError::
//! InvalidInstructionData` at runtime with no compile-time warning.
//!
//! This test computes the canonical 8-byte discriminators for all 3
//! instructions + 1 account + 2 events, then asserts byte-for-byte
//! equality against the hand-written IDL JSON. If the IDL is ever edited
//! with wrong discriminators, this test fires immediately.
//!
//! Source: P07-T2-01 (Sonnet CPI/DX).

use anchor_lang::solana_program::hash;
use serde_json::Value;

const IDL_JSON: &str = include_str!("../../../target/idl/alea_verifier.json");

fn canonical_discriminator(preimage: &str) -> [u8; 8] {
    let digest = hash::hash(preimage.as_bytes()).to_bytes();
    let mut out = [0u8; 8];
    out.copy_from_slice(&digest[..8]);
    out
}

fn idl_discriminator(idl: &Value, section: &str, name: &str) -> Vec<u8> {
    let arr = idl[section]
        .as_array()
        .unwrap_or_else(|| panic!("IDL missing section `{section}`"));
    let entry = arr
        .iter()
        .find(|e| e["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("IDL {section} missing entry `{name}`"));
    entry["discriminator"]
        .as_array()
        .unwrap_or_else(|| panic!("IDL {section}/{name} missing `discriminator`"))
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

fn assert_discriminator(idl: &Value, section: &str, name: &str, preimage: &str) {
    let expected = canonical_discriminator(preimage);
    let actual = idl_discriminator(idl, section, name);
    assert_eq!(
        actual.as_slice(),
        expected.as_slice(),
        "IDL {section}/{name} discriminator != sha256(\"{preimage}\")[..8]: \
         expected {:?}, got {:?}",
        expected.as_slice(),
        actual.as_slice()
    );
}

#[test]
fn idl_instruction_discriminators_match_anchor_convention() {
    let idl: Value = serde_json::from_str(IDL_JSON).expect("IDL must be valid JSON");

    assert_discriminator(&idl, "instructions", "initialize", "global:initialize");
    assert_discriminator(&idl, "instructions", "verify", "global:verify");
    assert_discriminator(&idl, "instructions", "update_config", "global:update_config");
}

#[test]
fn idl_account_discriminators_match_anchor_convention() {
    let idl: Value = serde_json::from_str(IDL_JSON).expect("IDL must be valid JSON");

    assert_discriminator(&idl, "accounts", "Config", "account:Config");
}

#[test]
fn idl_event_discriminators_match_anchor_convention() {
    let idl: Value = serde_json::from_str(IDL_JSON).expect("IDL must be valid JSON");

    assert_discriminator(&idl, "events", "BeaconVerified", "event:BeaconVerified");
    assert_discriminator(&idl, "events", "ConfigUpdated", "event:ConfigUpdated");
}
