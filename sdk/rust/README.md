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

```rust
use alea_sdk::{self, AleaVerifier};
use anchor_lang::solana_program::sysvar::clock::Clock;

const MAX_BEACON_AGE_SECONDS: u64 = 30; // reject drand rounds older than 30s

#[derive(Accounts)]
pub struct SettleMatch<'info> {
    // ... your program's own accounts ...

    /// Alea program for randomness verification
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

    let random_value = u64::from_le_bytes(randomness[0..8].try_into().unwrap());
    let winner = if random_value % 2 == 0 { player_a } else { player_b };

    // ... settle logic ...
    Ok(())
}
```

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

| Code | Name | Meaning |
|------|------|---------|
| 6000 | `InvalidSignature` | Pairing check failed — wrong sig for this round |
| 6001 | `InvalidG1Point` | Signature is not a canonical BN254 G1 point |
| 6002 | `RoundZero` | Round 0 is forbidden (drand genesis sentinel) |
| 6003 | `ChainHashMismatch` | Config chain hash does not match expected |
| 6004 | `NoSquareRoot` | SVDW map_to_point: no sqrt found (rare) |
| 6005 | `InvalidChainHash` | Chain hash is all-zeros |
| 6006 | `PairingError` | BPF alt_bn128 syscall error (infrastructure failure) |
| 6007 | `InvalidPubkeyG2` | G2 pubkey is all-zeros |
| 6008 | `InvalidPublicKey` | Reserved |
| 6009 | `ReturnDataMissing` | Reserved (unreachable under Pattern A) |
| 6010 | `InvalidGenesisTime` | Genesis time is zero |
| 6011 | `InvalidPeriod` | Period is zero |
| 2001 | Anchor `AccountNotInitialized` | Config PDA not initialized |

---

## Links

- GitHub: [alea-drand/alea](https://github.com/alea-drand/alea)
- Full docs site: Coming Phase 6
- License: Apache 2.0 — see [LICENSE](LICENSE)
- Maturity disclosures: [CAVEATS.md](CAVEATS.md)
