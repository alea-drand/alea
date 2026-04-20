# alea-sdk

**First production drand BN254 verifier on Solana. One-line CPI, free public good.**

Any Anchor program can receive cryptographically verified on-chain randomness with a single CPI call — no oracles, no callbacks, no off-chain coordination.

> **Warning:** Read [CAVEATS.md](CAVEATS.md) before integrating. This crate is live on Solana devnet. Mainnet deployment is pending.

---

## Install

```toml
[dependencies]
alea-sdk = "0.1"

# Required pin — without this, cargo check / cargo build-sbf fails on
# rustc < 1.95 (current stable is 1.94, Solana BPF is 1.89-dev). See
# Troubleshooting below for context. Re-check after every `cargo update`.
constant_time_eq = "=0.4.2"
```

or

```bash
cargo add alea-sdk
cargo add constant_time_eq@=0.4.2   # required transitive pin; see Troubleshooting
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

    // MANDATORY: verify the drand beacon via CPI — one line. Returns
    // alea_sdk::VerifiedRandomness (must_use wrapper) so a forgotten
    // `.into_inner()` / `.as_bytes()` produces a compile warning
    // instead of silently dropping the 32 bytes.
    let randomness = alea_sdk::cpi::verify(
        ctx.accounts.alea_program.to_account_info(),
        ctx.accounts.alea_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        round,
        signature,
    )?.into_inner();

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

The TypeScript SDK (`@alea-drand/sdk`) injects this automatically. Rust consumers must add it manually.

---

## Program IDs

| Cluster | Program ID |
|---------|-----------|
| Devnet  | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` |
| Mainnet | Pending (same vanity ID — cluster binding identical) |

Devnet-verified across 10 live rounds. Mainnet deployment pending.

---

## Error Codes

Canonical source: [`programs/alea-verifier/src/errors.rs`](https://github.com/alea-drand/alea/blob/main/programs/alea-verifier/src/errors.rs).
CI enforces table-to-enum coherence on every PR.

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
| 6008 | `WrongPubkey` | `Config.pubkey_g2` does not match `EXPECTED_EVMNET_G2_PUBKEY` — also emitted by `alea_sdk::cpi::verify`'s owner check on the config account |
| 6009 | `ReturnDataMissing` | **Reserved (unreachable under ADR 0030 Pattern A)** |
| 6010 | `InvalidGenesisTime` | `Config.genesis_time` does not match `EXPECTED_EVMNET_GENESIS_TIME` |
| 6011 | `InvalidPeriod` | `Config.period` does not match `EXPECTED_EVMNET_PERIOD` |
| 6012 | `UnauthorizedInit` | `initialize` signer does not equal the program's `upgrade_authority_address` |
| 2001 | Anchor `ConstraintHasOne` | `update_config` signer is not the stored config authority (framework code) |
| 3010 | Anchor `AccountNotSigner` | `authority` account passed without a signature (framework code, fires before 2001) |

---

## Troubleshooting

### `constant_time_eq@0.4.3 requires rustc 1.95` (both `cargo check` AND `cargo build-sbf`)

**This affects any Rust toolchain older than 1.95, not just BPF.** Even on
native `cargo check` / `cargo doc` with stable rustc 1.94 (the current
release at the time of writing), the resolver picks `constant_time_eq
0.4.3` which requires rustc 1.95+. The Solana BPF toolchain's embedded
rustc is ~1.89-dev so BPF builds hit it too.

Workaround — pin the offending transitive in your own `Cargo.toml`:

```toml
[dependencies]
alea-sdk = "0.1"

# Pin this transitive to a version that compiles on both current stable
# rustc and Solana BPF toolchain's 1.89-dev:
constant_time_eq = "=0.4.2"
```

Re-verify after every `cargo update` — if cargo bumps `constant_time_eq`
past 0.4.2 the pin gets silently overridden unless you use `=0.4.2`
(exact version, not `0.4.2` which is semver-compatible).

### Anchor's `declare_id!` macro — misleading error cascade on bad base58 length

If you copy the Quick Start and paste a placeholder program ID like
`MyProg111111111111111111111111111111111`, you'll see:

```
error: pubkey array is not 32 bytes long: len=N
error[E0425]: cannot find value `ID` in crate `crate`
  = help: consider importing this module: `anchor_lang::system_program::ID`
```

**Ignore the rustc helper suggestion about `system_program::ID`.** It's a
red herring — rustc can't see that the cascade is caused by the first
error (a proc-macro panic inside `declare_id!`) and guesses at imports.
The actual fix is: `declare_id!` requires exactly a **44-character
base58-encoded ed25519 pubkey** (32 bytes = 44 base58 chars). Generate a
real one with:

```bash
solana-keygen new --outfile /tmp/my-program-id.json --no-bip39-passphrase --force
solana-keygen pubkey /tmp/my-program-id.json
```

This is an Anchor UX issue, not an Alea-specific one, but new consumers
hit it on first integration.

This pattern applies to any transitive crate that assumes a newer rustc than Solana's BPF toolchain ships. If a new `cargo build-sbf` failure surfaces on a future crate, the fix is the same: pin the offending dep to an older compatible version in your consumer's `Cargo.toml`.

### `anchor build` fails with `E0599: no method named 'source_file'`

Anchor 0.30.1's `anchor-syn` crate calls a proc-macro2 API removed in 1.0.82+. This is an Anchor issue, not an `alea-sdk` issue, but consumers will hit it. Either:
- Pin proc-macro2 to `<=1.0.81` in your workspace
- Or use `cargo-build-sbf` directly + hand-managed IDL (see `programs/alea-verifier/` workflow)

### Compute budget exceeded / "Program failed to complete"

Every tx that CPIs into Alea MUST include `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. Solana's default is 200K; Alea needs up to 454K, plus consumer headroom. The [`@alea-drand/sdk` TypeScript SDK](https://npmjs.com/package/@alea-drand/sdk) injects this automatically; Rust consumers building raw txs must add it manually.

### `AleaError::WrongPubkey` (6008) when the on-chain Config looks correct

This can come from two code paths:
1. On-chain `verify` handler — `Config.pubkey_g2 != EXPECTED_EVMNET_G2_PUBKEY` (wrong-chain init); redeploy with the correct `chain_hash` / `pubkey_g2`
2. `alea_sdk::cpi::verify` helper — the supplied `config` account's owner is not `alea_sdk::PROGRAM_ID` (defense-in-depth check for non-Anchor callers)

If you're an Anchor program user with `#[account(seeds = [b"config"], bump, seeds::program = alea_program.key())]` on your config, the second case is not reachable for you — the PDA re-derivation catches it first.

---

## Links

- GitHub: [alea-drand/alea](https://github.com/alea-drand/alea)
- License: Apache 2.0 — see [LICENSE](LICENSE)
- Maturity disclosures: [CAVEATS.md](CAVEATS.md)
