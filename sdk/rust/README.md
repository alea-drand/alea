# alea-sdk

**First production drand BN254 verifier on Solana. One-line CPI, free public good.**

Any Anchor program can receive cryptographically verified on-chain randomness with a single CPI call — no oracles, no callbacks, no off-chain coordination.

> **Warning:** Read [CAVEATS.md](CAVEATS.md) before integrating. This crate is live on Solana devnet with an internal audit passing (avg 8.66/10 across 10 Claude + 5 Codex persona rounds); external paid firm review and mainnet deployment are Phase 5 gates.

---

## Install

```toml
[dependencies]
alea-sdk = "0.1"
```

or

```
cargo add alea-sdk
```

---

## Quick Start — Palestra-Style Integration

```rust,ignore
use alea_sdk::{self, AleaVerifier};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::clock::Clock;

const MAX_BEACON_AGE_SECONDS: u64 = 30; // reject drand rounds older than 30s

#[error_code]
pub enum YourError {
    #[msg("Beacon is too old; use a fresher drand round")]
    StaleBeacon,
}

#[derive(Accounts)]
pub struct SettleMatch<'info> {
    // ... your program's own accounts (e.g., game state, players) ...
    pub game: Account<'info, YourGameState>, // example — your type here

    /// Alea program for randomness verification.
    pub alea_program: Program<'info, alea_sdk::AleaVerifier>,

    /// Alea config PDA — MUST include `seeds::program` or your program is
    /// vulnerable to fake-config substitution (ADR 0034). Anchor does NOT
    /// enforce the program ownership check without this constraint.
    #[account(
        seeds = [b"config"],
        bump,
        seeds::program = alea_program.key(),   // ← MANDATORY. Do not remove.
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,

    /// Transaction payer — passed to Alea's verify instruction.
    pub payer: Signer<'info>,

    /// Clock sysvar for is_round_recent() recency check (MANDATORY).
    pub clock: Sysvar<'info, Clock>,
}

pub fn settle_match(
    ctx: Context<SettleMatch>,
    round: u64,
    signature: [u8; 64],
) -> Result<()> {
    // MANDATORY: reject stale beacons BEFORE verification.
    // Without this, an attacker can replay a known-old beacon to bias outcomes.
    require!(
        alea_sdk::is_round_recent(
            round,
            &ctx.accounts.alea_config,
            &ctx.accounts.clock,
            MAX_BEACON_AGE_SECONDS,
        ),
        YourError::StaleBeacon,
    );

    // MANDATORY: verify the drand beacon via CPI — one line.
    let randomness = alea_sdk::cpi::verify(
        ctx.accounts.alea_program.to_account_info(),
        ctx.accounts.alea_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        round,
        signature,
    )?;

    // MANDATORY: read the result IMMEDIATELY.
    // Solana return data is overwritten by any subsequent CPI call.
    // Do NOT insert token transfers or other CPIs between verify and this line.
    //
    // Convert the leading 8 bytes to a u64. The try_into unwrap is
    // infallible here: randomness is always [u8; 32] and 0..8 is a valid
    // slice. Do NOT cargo-cult `.unwrap()` into consumer code that reads
    // user input — this is a known-safe path only.
    let random_value = u64::from_le_bytes(randomness[0..8].try_into().unwrap());
    let winner_index = (random_value % 2) as usize;

    // ... settle logic using winner_index against your game's player list ...
    msg!("Alea random_value={} winner_index={}", random_value, winner_index);
    Ok(())
}
```

> The example is annotated `rust,ignore` because it references a hypothetical `YourGameState` account type. See [`programs/example-lottery/`](https://github.com/alea-drand/alea/tree/main/programs/example-lottery) in the repo for a complete, compiling reference consumer.

---

## Security: Mandatory Constraints for Consumer Programs

**Two constraints are MANDATORY for any consumer program. Omitting either ships an exploitable program.**

### 1. `seeds::program = alea_program.key()` on `alea_config`

```rust
#[account(
    seeds = [b"config"],
    bump,
    seeds::program = alea_program.key(),   // ← cannot be omitted
)]
pub alea_config: Account<'info, alea_sdk::Config>,
```

Without this, an attacker can substitute a fake Config PDA owned by a different program. Anchor re-derives the PDA using Alea's program ID as the signer only when `seeds::program` is present — it is NOT enforced by default. This guards against total compromise of your randomness source. (ADR 0034)

### 2. `is_round_recent()` before trusting randomness

```rust
require!(
    alea_sdk::is_round_recent(round, &ctx.accounts.alea_config, &ctx.accounts.clock, 30),
    YourError::StaleBeacon,
);
```

Without recency enforcement, an attacker can replay an old drand round whose randomness they already know to bias a resolution. Alea's `verify` accepts any round — recency is the consumer's responsibility.

### 3. (Privacy-sensitive only) Route through program PDA signer

For applications where the fact that a user is consulting randomness leaks game state (anonymous lotteries, sealed-bid auctions), route the verify CPI through a program-owned PDA signer rather than the end-user wallet. The on-chain `BeaconVerified` event records the payer. Public-by-design applications can use the end-user wallet directly.

---

## CPI Return Data Ordering Warning

Solana's return data is single-slot — each CPI call **overwrites** the previous value. Capture the randomness immediately:

```rust
// CORRECT — capture first, then other CPIs
let randomness = alea_sdk::cpi::verify(/* args */)?;
token::transfer(transfer_ctx, amount)?;  // safe

// WRONG — overwrites Alea's return data before you read it
token::transfer(transfer_ctx, amount)?;
let randomness = alea_sdk::cpi::verify(/* args */)?;  // stale
```

---

## Compute Budget Requirement

Every transaction calling Alea **MUST** include a compute budget instruction of at least 900,000 CU. Solana's default is 200K; Alea's verify needs up to 454K, plus consumer headroom.

```rust
// Add this before your instruction in every transaction:
let cu_ix = ComputeBudgetInstruction::set_compute_unit_limit(900_000);
```

The TypeScript SDK (`@alea/sdk`) injects this automatically. Rust consumers must add it manually.

---

## Program IDs

| Cluster | Program ID |
|---------|-----------|
| Devnet  | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` |
| Mainnet | Pending Phase 5 (same vanity ID — cluster binding identical) |

Devnet-verified across 10 live rounds. Mainnet traffic begins Phase 5.

---

## Error Codes

Canonical source: [`programs/alea-verifier/src/errors.rs`](https://github.com/alea-drand/alea/blob/main/programs/alea-verifier/src/errors.rs).
CI enforces table-to-enum coherence on every PR (Phase 6).

| Code | Name | Meaning |
|------|------|---------|
| 6000 | `InvalidSignature` | BLS pairing check returned non-1 — signature does not attest this (round, pubkey) pair |
| 6001 | `InvalidG1Point` | Signature bytes decode but are not on the BN254 G1 curve (y² ≠ x³ + 3 mod p) |
| 6002 | `RoundZero` | Round must be > 0 (drand's round 0 is a sentinel, never emitted) |
| 6003 | `InvalidFieldElement` | **Reserved (unreachable in v1)** — do not treat as retryable |
| 6004 | `NoSquareRoot` | `hash_round_to_g1` exhausted all SVDW candidates; constant corruption or syscall regression (not attacker-reachable) |
| 6005 | `InvalidG2Point` | **Reserved (unreachable under ADR 0027 fallback path)** — do not retry |
| 6006 | `PairingError` | `alt_bn128_pairing` syscall returned Err or wrong-length output (infrastructure) |
| 6007 | `WrongChainHash` | `Config.chain_hash` does not match `EXPECTED_EVMNET_CHAIN_HASH` (wrong-chain deployment) |
| 6008 | `WrongPubkey` | `Config.pubkey_g2` does not match `EXPECTED_EVMNET_G2_PUBKEY` — also emitted by `alea_sdk::cpi::verify`'s owner check on the config account (T1-08, Phase 4.5) |
| 6009 | `ReturnDataMissing` | **Reserved (unreachable under ADR 0030 Pattern A)** |
| 6010 | `InvalidGenesisTime` | `Config.genesis_time` does not match `EXPECTED_EVMNET_GENESIS_TIME` |
| 6011 | `InvalidPeriod` | `Config.period` does not match `EXPECTED_EVMNET_PERIOD` |
| 6012 | `UnauthorizedInit` | `initialize` signer does not equal the program's `upgrade_authority_address` |
| 2001 | Anchor `ConstraintHasOne` | `update_config` signer is not the stored config authority (framework code) |
| 3010 | Anchor `AccountNotSigner` | `authority` account passed without a signature (framework code, fires before 2001) |

---

## Troubleshooting

### `cargo build-sbf` fails on `constant_time_eq@0.4.3 requires rustc 1.95`

The Solana BPF toolchain's embedded rustc is ~1.89-dev, lagging the Rust
ecosystem by 6+ minor versions. Modern crates.io crates sometimes assume
newer rustc. Workaround — pin the offending dep in your own `Cargo.toml`:

```toml
[dependencies]
alea-sdk = "0.1"

# Pin sub-dep to a BPF-compatible version:
constant_time_eq = "=0.4.2"
```

See [`2026-04-19-solana-bpf-rustc-lag-external-consumers`](https://github.com/alea-drand/alea/blob/main/build-spec/decisions/) for the full list of commonly-affected transitives (updated as Solana's toolchain evolves).

### `anchor build` fails with `E0599: no method named 'source_file'`

Anchor 0.30.1's `anchor-syn` crate calls a proc-macro2 API removed in 1.0.82+. This is an Anchor issue, not an `alea-sdk` issue, but consumers will hit it. Either:
- Pin proc-macro2 to `<=1.0.81` in your workspace
- Or use `cargo-build-sbf` directly + hand-managed IDL (see `programs/alea-verifier/` workflow)

### Compute budget exceeded / "Program failed to complete"

Every tx that CPIs into Alea MUST include `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. Solana's default is 200K; Alea needs up to 454K, plus consumer headroom. The [`@alea/sdk` TypeScript SDK](https://npmjs.com/package/@alea/sdk) injects this automatically; Rust consumers building raw txs must add it manually.

### `AleaError::WrongPubkey` (6008) when the on-chain Config looks correct

This can come from two code paths:
1. On-chain `verify` handler — `Config.pubkey_g2 != EXPECTED_EVMNET_G2_PUBKEY` (wrong-chain init); redeploy with the correct `chain_hash` / `pubkey_g2`
2. `alea_sdk::cpi::verify` helper — the supplied `config` account's owner is not `alea_sdk::PROGRAM_ID` (defense-in-depth check added Phase 4.5 T1-08 for non-Anchor callers)

If you're an Anchor program user with `#[account(seeds = [b"config"], bump, seeds::program = alea_program.key())]` on your config, the second case is not reachable for you — the PDA re-derivation catches it first.

---

## Links

- GitHub: [alea-drand/alea](https://github.com/alea-drand/alea)
- Full docs site: Coming Phase 6
- License: Apache 2.0 — see [LICENSE](LICENSE)
- Maturity disclosures: [CAVEATS.md](CAVEATS.md)
