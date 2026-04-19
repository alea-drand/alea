# Alea

On-chain [drand](https://drand.love) BN254 verifier for Solana. Apache 2.0, free public good.

[![CI](https://github.com/alea-drand/alea/actions/workflows/test.yml/badge.svg)](https://github.com/alea-drand/alea/actions/workflows/test.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/alea-sdk.svg)](https://crates.io/crates/alea-sdk)
[![npm](https://img.shields.io/npm/v/@alea-drand/sdk.svg)](https://www.npmjs.com/package/@alea-drand/sdk)
[![drand](https://img.shields.io/badge/powered%20by-drand-ff6b6b.svg)](https://drand.love)

Any Solana program can call `alea_sdk::cpi::verify(program, config, payer, round, signature)` and get 32 bytes of cryptographically verified randomness in a single transaction — no callbacks, no keepers, no off-chain coordinators, no protocol fees.

---

## Table of Contents

- [What Alea Is](#what-alea-is)
- [Why Alea](#why-alea)
- [Status](#status)
- [Use Cases](#use-cases)
- [Quick Start](#quick-start)
- [How It Works](#how-it-works)
- [Architecture](#architecture)
- [Packages](#packages)
- [API Reference](#api-reference)
- [Program Addresses](#program-addresses)
- [Error Codes](#error-codes)
- [Security](#security)
- [Testing & Validation](#testing--validation)
- [Governance & Roadmap](#governance--upgrade-roadmap)
- [Testing on Devnet](#testing-on-devnet)
- [FAQ](#faq)
- [Prior Art & Credits](#prior-art--credits)
- [Contributing](#contributing)
- [License](#license)

---

## What Alea Is

Alea is a **cryptographic randomness primitive** for Solana. It verifies randomness beacons from [drand](https://drand.love) — the League of Entropy's threshold-signed public randomness network — directly on-chain, using Solana's `alt_bn128_pairing` syscall.

- **drand** is a decentralized network run by the League of Entropy (a coalition of universities, companies, and non-profits including Cloudflare, Protocol Labs, EPFL and others) that jointly produces a publicly verifiable randomness beacon every 3 seconds using BLS threshold signatures on the BN254 curve.
- **Alea** verifies those beacons on-chain through full BN254 pairing math. If the pairing check passes, the signature is authentic and the 32 bytes returned are the canonical randomness for that round.
- **The output is deterministic**: any two consumers verifying the same round get the same 32 bytes. This makes Alea well-suited for public-draw semantics (lotteries, tournaments, fair launches) and poorly suited for per-request unique-seed VRF (see the [FAQ](#faq) for per-caller randomness derivation patterns).
- **It is stateless**: Alea doesn't store beacons. Each `verify` call takes the round number and signature from the caller, checks the pairing, and returns randomness. A single on-chain `Config` PDA holds the drand public key + chain hash — read-only during verification.

This is not a VRF service in the ORAO/Switchboard sense (per-request oracle). It's more like a **verifiable public-randomness oracle** — closer in spirit to Ethereum's `RANDAO` (except cryptographically auditable on-chain rather than chain-derived).

---

## Why Alea

| Existing option | Cost per call | Friction | Provenance | Trust model |
|-----------------|---------------|----------|------------|-------------|
| [ORAO VRF](https://orao.network) | ~$0.15 | Request-then-callback (2 tx) | Single-operator attestation | Trust the operator |
| [Switchboard](https://switchboard.xyz) randomness | ~$0.01+ | Oracle round trip | Oracle committee attestation | Trust the committee |
| [MagicBlock VRF](https://docs.magicblock.gg) | Not publicly disclosed | Request-then-callback (2 tx) | Program-signed callback | Trust the VRF operator |
| Commit-reveal (DIY) | 0 | 2 tx + coordinator logic | No cryptographic provenance | Trust your own coordinator |
| **Alea** | **$0** (~0.000005 SOL tx fee) | **1 CPI call, single tx** | **drand threshold sig, verified on-chain via BN254 BLS** | **Trust drand's League-of-Entropy threshold** |

**What makes Alea different:** the other options on this row are oracle-based — a designated operator or committee produces the randomness and attests to it. Consumers trust that attestation. Alea has no operator: the randomness is the drand beacon itself, and Solana verifies the BN254 pairing on-chain before returning the bytes. The trust surface is drand's threshold-signer set, not an intermediate oracle.

**Alea trades off**: you can't get per-caller unique randomness without consumer-side derivation (see [FAQ](#faq)). For public-draw semantics, that's not a trade-off — it's a feature. Multiple consumers watching the same round arrive at the same bytes independently, which means the "fair draw" is auditable by anyone.

---

## Status

**v0.1.0 — devnet release, April 2026.**

- **Devnet:** Live at program `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`. Verified end-to-end against live drand rounds.
- **Mainnet:** Phase 5 gate — pending (a) external paid security audit, (b) Squads 2-of-3 multisig transition, (c) a remaining open BPF error-path runtime test.
- **Published SDKs:**
  - [`alea-verifier v0.1.0`](https://crates.io/crates/alea-verifier) on crates.io
  - [`alea-sdk v0.1.0`](https://crates.io/crates/alea-sdk) on crates.io
  - [`@alea-drand/sdk v0.1.0`](https://www.npmjs.com/package/@alea-drand/sdk) on npm

Read [CHANGELOG.md](CHANGELOG.md) for release notes and [`sdk/rust/CAVEATS.md`](sdk/rust/CAVEATS.md) + [`sdk/typescript/CAVEATS.md`](sdk/typescript/CAVEATS.md) for maturity disclosures before integrating in production.

---

## Use Cases

**Good fit:**
- Lotteries and raffles where everyone sees the same draw
- Tournament brackets and seeding (public, auditable)
- On-chain games with public-draw semantics (slots, dice, card games with open outcomes)
- Fair launch and mint-order randomization
- Governance sortition (picking N delegates from M candidates)
- NFT trait reveal with provable fairness

**Needs consumer-side derivation** (see [FAQ](#faq) for the `per_user = sha256(round_randomness || user_pubkey)` pattern):
- Per-user unique randomness
- Private-bid auctions (commit-reveal + Alea as the reveal trigger)

**Not a fit:**
- High-frequency randomness where 3-second drand rounds are too slow
- Truly private randomness (drand beacons are public by design — consumers who know the round number learn the randomness)

---

## Quick Start

```bash
cargo add alea-sdk
# or: npm install @alea-drand/sdk @solana/web3.js @coral-xyz/anchor
```

### Rust CPI (on-chain composition)

```rust
use alea_sdk::{self, AleaVerifier};

#[derive(Accounts)]
pub struct ResolveGame<'info> {
    pub alea_program: Program<'info, alea_sdk::AleaVerifier>,
    #[account(
        seeds = [b"config"],
        bump,
        seeds::program = alea_program.key(),  // MANDATORY (fake-config guard)
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,
    pub payer: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
    // ... your accounts ...
}

pub fn resolve_game(ctx: Context<ResolveGame>, round: u64, signature: [u8; 64]) -> Result<()> {
    // MANDATORY: reject stale beacons before trusting randomness
    require!(
        alea_sdk::is_round_recent(round, &ctx.accounts.alea_config, &ctx.accounts.clock, 30),
        GameError::StaleBeacon,
    );

    // One-line CPI. Returns VerifiedRandomness (must_use wrapper so a
    // forgotten `.into_inner()` produces a compile warning rather than
    // silently dropping the 32 bytes).
    let randomness = alea_sdk::cpi::verify(
        ctx.accounts.alea_program.to_account_info(),
        ctx.accounts.alea_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        round,
        signature,
    )?.into_inner();

    // Use the 32-byte randomness. Capture BEFORE any subsequent CPI —
    // Solana's return-data slot is single-use.
    let winner_index = u64::from_le_bytes(randomness[0..8].try_into().unwrap()) % 2;
    // ... settle logic ...

    Ok(())
}
```

**Mandatory consumer constraints** (omitting either ships an exploitable program):

1. **`seeds::program = alea_program.key()`** on the `alea_config` account — prevents fake-config substitution attacks where an attacker passes a PDA owned by their own program with attacker-chosen public keys.
2. **`is_round_recent(round, config, clock, max_age)`** before trusting randomness — prevents replay of old drand rounds whose randomness is publicly known.

Both are documented in [`sdk/rust/README.md`](sdk/rust/README.md) §Security and demonstrated in the canonical [`programs/example-lottery/`](programs/example-lottery/).

### TypeScript / Node (off-chain fetch + submit)

```typescript
import { getVerifiedRandomness } from "@alea-drand/sdk";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const signer = Keypair.fromSecretKey(/* your keypair bytes */);

// One-liner: fetches the latest drand beacon, submits verify tx, returns 32 bytes.
const randomness: Uint8Array = await getVerifiedRandomness({
  connection,
  signer,
  // round defaults to latest drand round — pass a bigint to pin a specific one
});

console.log("Verified randomness (hex):", Buffer.from(randomness).toString("hex"));
```

Browser (Vite / webpack / Next.js App Router / esbuild) works out of the box — no `fs` polyfills, zero Node-only imports in the published bundle. See [`sdk/typescript/README.md`](sdk/typescript/README.md) for wallet-adapter integration + the browser quick-start.

**v0.1.x is devnet-only.** The SDK's program ID is cluster-agnostic — `DEVNET_PROGRAM_ID` and `MAINNET_PROGRAM_ID` point to the same bytes. Your `Connection` object determines cluster. A mainnet `Connection` before Phase 5 fails at the Solana RPC layer with "program not found" — Solana itself is the safety rail.

---

## How It Works

1. **Your program or app calls** `alea_sdk::cpi::verify(program, config, payer, round, signature)` (Rust on-chain) or `getVerifiedRandomness({ connection, signer })` (TypeScript off-chain → Solana tx).
2. **Alea's on-chain program computes** `msg_hash = keccak256(round_be_u64)`.
3. **Alea runs full SVDW hash-to-curve** (Shallue–van de Woestijne) on Solana BPF to map `msg_hash` into a G1 point `M ∈ BN254_G1`. This is the critical path: ~250–400K compute units.
4. **Alea invokes Solana's `alt_bn128_pairing` syscall** to verify `e(σ, G2_gen) == e(M, pubkey_G2)` where:
   - `σ` is the caller-supplied 64-byte signature (drand beacon)
   - `pubkey_G2` is drand's public key, stored in the `Config` PDA
   - `G2_gen` is BN254's generator
   - Pairing check costs ~48K CU
5. **On success**, Alea returns `sha256(signature_bytes)` as 32 bytes — matches drand's published `randomness` field byte-for-byte (drand's `bls-bn254-unchained-on-g1` scheme).

No off-chain steps. No hinting. No trusted coordinator. Pure cryptographic verification.

### Total compute budget

Alea verify consumes **up to ~454K CU** worst-case. Solana's default per-instruction budget is 200K, so **every transaction calling Alea MUST include** `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. The TypeScript SDK injects this automatically. Rust consumers building raw transactions must add it manually.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  drand League of Entropy (threshold signed, 3s period)          │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ fetchBeacon() — 5 endpoints with fallback
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Your application                           │
│                                                                 │
│  @alea-drand/sdk (TypeScript)  OR  alea-sdk (Rust CPI)          │
│    ├─ fetch drand beacon           ├─ require!(is_round_recent) │
│    ├─ inject ComputeBudget 900K    ├─ alea_sdk::cpi::verify()   │
│    └─ sign + sendRawTransaction    └─ capture randomness        │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ Solana RPC → tx
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                  alea-verifier (on-chain)                       │
│                                                                 │
│  1. Read Config PDA (drand pubkey_g2, chain_hash, period)       │
│  2. keccak256(round_be_u64) → 32-byte hash                      │
│  3. SVDW hash-to-curve → G1 point M              [~250-400K CU] │
│  4. alt_bn128_pairing([σ, G2_gen, -M, pubkey_g2]) == 1  [~48K]  │
│  5. Emit BeaconVerified event, return sha256(signature)         │
└─────────────────────────────────────────────────────────────────┘
                            │
                            │ return data = 32 bytes
                            ▼
                  Consumer program uses randomness
                  (must capture before any other CPI)
```

**Stateless design**: one Config PDA (`seeds = [b"config"]`) holds drand's G2 public key, chain_hash, genesis_time, and period. No beacon storage, no request/response cycle, no off-chain coordinator.

---

## Packages

| Package | Registry | Purpose | Size |
|---------|----------|---------|------|
| [`alea-verifier`](https://crates.io/crates/alea-verifier) | crates.io | On-chain Anchor program, imported as library | ~180 KB BPF .so |
| [`alea-sdk`](https://crates.io/crates/alea-sdk) | crates.io | Rust CPI helper for consumer programs | thin wrapper |
| [`@alea-drand/sdk`](https://www.npmjs.com/package/@alea-drand/sdk) | npm | TypeScript SDK: fetch drand + submit verify tx | 27.9 KB packed, 98.2 KB unpacked, ESM-only |

All three pinned to the same v0.1.0 (per ADR 0028 exact-pin policy).

---

## API Reference

### Rust (`alea-sdk`)

```rust
// Types
pub struct AleaVerifier;               // Program type: Program<'info, AleaVerifier>
pub struct Config { ... };             // Config PDA state (re-exported from alea-verifier)
pub enum AleaError { ... };            // 6000–6012 error codes (see below)
pub struct VerifiedRandomness([u8; 32]);  // #[must_use] wrapper — can't silently drop

// Constants
pub const PROGRAM_ID: Pubkey;          // ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U

// Functions
pub fn config_pda(program_id: &Pubkey) -> (Pubkey, u8);
pub fn is_round_recent(
    round: u64,
    config: &Config,
    clock: &Clock,
    max_age_seconds: u64,
) -> bool;

// CPI module
pub mod cpi {
    pub fn verify<'info>(
        alea_program: AccountInfo<'info>,
        config: AccountInfo<'info>,
        payer: AccountInfo<'info>,
        round: u64,
        signature: [u8; 64],
    ) -> Result<VerifiedRandomness>;
}
```

### TypeScript (`@alea-drand/sdk`)

```typescript
// High-level entry point
getVerifiedRandomness(options: {
  connection: Connection;
  signer: Keypair | Wallet;
  programId?: PublicKey;                // defaults to DEVNET_PROGRAM_ID
  round?: bigint;                       // defaults to latest drand round
  computeUnits?: number;                // defaults to 900_000
  signal?: AbortSignal;                 // cancel mid-fetch or pre-broadcast
  skipPreflight?: boolean;              // default true (required for pairing CU)
}): Promise<Uint8Array>;

// Lower-level
verifyDrandBeacon(args): Promise<Uint8Array>;
fetchBeacon(round?, { signal? }): Promise<DrandBeacon>;
createVerifyInstruction({ round, signature, payer, programId? }): TransactionInstruction;

// Pure helpers
getCurrentRound(): bigint;
getRoundAt(timestamp: bigint): bigint;
isRoundRecent(round, config, clock, maxAgeSeconds): boolean;
getConfigAddress(programId?): PublicKey;

// Constants + errors
DRAND_CHAIN_HASH, DRAND_GENESIS_TIME, DRAND_PERIOD, DRAND_ENDPOINTS;
DEVNET_PROGRAM_ID, MAINNET_PROGRAM_ID;  // same bytes; cluster picked via Connection
AleaError, ERRORS;
```

See [`sdk/typescript/README.md`](sdk/typescript/README.md) for complete signatures + usage examples.

---

## Program Addresses

| Cluster | Program ID | Config PDA |
|---------|-----------|------------|
| **Devnet** | [`ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`](https://explorer.solana.com/address/ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U?cluster=devnet) | `6anALRxD98Tw7zbA9d5i4NJfTvxDsNBHohHVJWxv2Xm8` |
| **Mainnet** | Same vanity ID (deploys in Phase 5) | TBD (derived from same seeds + program ID) |
| **Upgrade authority (v1)** | `9cPWdtoR7sW7VVYxfrJZ9ekxX2fZctskXn3L4BSmafcc` (deployer keypair) | — |
| **Binary SHA256 (devnet)** | `8965062489fdcdbb538597545fc6692f3f580d770d34f2d42000a70560984b1c` | — |

---

## Error Codes

Canonical source: [`programs/alea-verifier/src/errors.rs`](programs/alea-verifier/src/errors.rs). CI enforces `idl-diff` to prevent silent schema drift.

| Code | Name | Meaning | Retryable? |
|------|------|---------|------------|
| 2001 | `ConstraintHasOne` (Anchor) | `update_config` signer not config authority | No |
| 3010 | `AccountNotSigner` (Anchor) | `authority` passed without signature | No |
| 6000 | `InvalidSignature` | BLS pairing check failed for this (round, sig) | No |
| 6001 | `InvalidG1Point` | Signature bytes not on BN254 G1 curve | No |
| 6002 | `RoundZero` | Round 0 forbidden (drand sentinel) | No |
| 6003 | `InvalidFieldElement` | **Reserved** — unreachable in v1 | No |
| 6004 | `NoSquareRoot` | SVDW exhausted candidates (infra failure) | No |
| 6005 | `InvalidG2Point` | **Reserved** — unreachable under ADR 0027 | No |
| 6006 | `PairingError` | `alt_bn128_pairing` syscall failed (infra) | No |
| 6007 | `WrongChainHash` | Config.chain_hash ≠ evmnet (wrong-chain deploy) | No |
| 6008 | `WrongPubkey` | Config.pubkey_g2 ≠ evmnet; also emitted by SDK's CPI owner check | No |
| 6009 | `ReturnDataMissing` | **Reserved** — unreachable under ADR 0030 Pattern A | No |
| 6010 | `InvalidGenesisTime` | Config.genesis_time ≠ evmnet | No |
| 6011 | `InvalidPeriod` | Config.period ≠ evmnet | No |
| 6012 | `UnauthorizedInit` | `initialize` signer ≠ program upgrade authority | No |
| **6100** (SDK only) | `DrandFetchFailed` | All drand endpoints failed after retries | Yes — transient network |
| **6101** (SDK only) | `DrandRoundMismatch` | Endpoint returned different round than requested | Yes — try next endpoint |
| **6102** (SDK only) | `InvalidInput` | SDK-level input validation (bad hex, wrong sig length, etc.) | No |
| **6103** (SDK only) | `Aborted` | Caller AbortSignal fired pre-broadcast | No |

---

## Security

- **Report vulnerabilities:** [GitHub Security Advisory](https://github.com/alea-drand/alea/security/advisories/new) (preferred) or `security@alea.so` (fallback — mail server provisioning pending; use GitHub until then). Pre-multisig response is best-effort within a few days for P0 issues; post-transition the target is 72h ack / 7d triage / 30d P0 fix. Details in [`.github/SECURITY.md`](.github/SECURITY.md).
- **Bug bounty:** intent documented; activation post-grant. Meaningful reports get credit in release notes (with permission) and potential non-monetary recognition.
- **Mandatory consumer constraints** (omitting either ships an exploitable program):
  1. `seeds::program = alea_program.key()` on the Config PDA — fake-config defense
  2. `is_round_recent()` before trusting randomness — anti-replay
  3. Capture return data immediately — Solana's return-data slot is single-use
- **Defense-in-depth**: `alea_sdk::cpi::verify()` also asserts `config.owner == PROGRAM_ID` at the wrapper layer (~200 CU) — catches non-Anchor callers who bypass the `seeds::program` check.
- **Supply-chain**: `cargo-deny` (licenses + advisories + bans + sources) + `npm audit` + `trufflehog` secret-scan run on every PR and weekly cron. Published npm tarballs carry Sigstore provenance attestations.

---

## Testing & Validation

**Unit + integration tests:** 70+ Rust unit/integration tests covering SVDW hash-to-curve, G1/G2 on-curve checks, pairing verification, `is_round_recent` boundary cases, and PDA derivation. 37+ TypeScript unit tests covering the drand client, instruction builder, error extraction, and input validation. Full suite runs in CI on every PR.

**Live devnet integration:** end-to-end verification against live drand rounds on devnet (`cargo test -- --ignored` gates these behind explicit opt-in). Fixture-based regressions on canonical round-1 + round-9337227 beacons.

**Fuzzing** — 5 parallel cargo-fuzz targets covering the full cryptographic pipeline:

| Target | Coverage |
|--------|----------|
| `verify_beacon` | end-to-end verify (round + signature → randomness) |
| `hash_to_g1` | full SVDW hash-to-curve |
| `on_curve_g1` | G1 on-curve validation |
| `hash_to_field_canonicity` | Fq field-element canonicity |
| `pairing_buffer_parses` | pairing input deserialization |

Final campaign (April 2026): **22.05 billion iterations across all 5 targets in 13 hours wall time with 0 crashes, 0 memory errors, 0 timeouts** (libFuzzer `-fork=3` per target on 18-core hardware). An earlier corpus-seeded pilot added 1.77 billion iterations across the three original targets — combined total **23.82 billion iterations**. Proof tarballs (per-target coverage HTML + metadata + SHA256 sums) are attached to the [`v0.2.0-audit-passed`](https://github.com/alea-drand/alea/releases/tag/v0.2.0-audit-passed) GitHub release.

**Supply chain:** `cargo-deny` (licenses + advisories + bans + sources) + `npm audit` + `trufflehog` secret-scan run on every PR and weekly cron.

**External paid security audit** — not yet performed. Required before the Phase 5 mainnet deploy.

---

## Governance & Upgrade Roadmap

Alea is stateless: the on-chain program holds no user funds (no TVL). The upgrade-authority roadmap below is about who can modify the verifier program itself.

| Phase | Authority | Trigger | Status |
|-------|-----------|---------|--------|
| v1 (devnet today, mainnet at Phase 5) | Deployer keypair | Initial release | **Current** |
| v2 | Squads 2-of-3 multisig | Within 90 days of mainnet deploy, or at the first external paid audit (whichever first) | Pending Phase 5 |
| v3 | Immutable (authority zeroed) | After external audit clears and the program operates without critical bugs for a meaningful period on mainnet | Planned |

Co-signers for the multisig will be named when Phase 5 completes.

**Interface stability guarantee**: the `verify` instruction signature, `Config` account layout, `Verify` accounts struct, return-data format, and `BeaconVerified` event schema are **frozen forever** for the mainnet program ID. Additive-only changes are welcome at minor versions; breaking changes require a new program ID (new deployment, not an upgrade). CI enforces this via the `idl-diff` check on every PR.

---

## Testing on Devnet

Before integrating into production, run the SDK against live devnet:

```bash
# 1. Install the SDKs
cargo add alea-sdk
npm install @alea-drand/sdk @solana/web3.js @coral-xyz/anchor

# 2. Get a devnet keypair + some SOL
solana-keygen new --outfile ~/.config/solana/alea-test.json
solana airdrop 1 ~/.config/solana/alea-test.json --url devnet
# If the solana-labs faucet is dry (common), use Discord fallbacks:
#   - 76 Devs: https://discord.com/invite/76Devs
#   - LamportDAO: https://discord.gg/LamportDAO

# 3. Run either quick-start from above against devnet
# Expected: 32 bytes of verified randomness in hex, within ~10 seconds
```

Live devnet program: [`ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`](https://explorer.solana.com/address/ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U?cluster=devnet).

A complete reference consumer (commit-reveal lottery with all mandatory + SHOULD security constraints) lives at [`programs/example-lottery/`](programs/example-lottery/src/lib.rs).

---

## FAQ

**Is this a VRF?**
Not in the classical (per-request-unique) sense. Alea verifies a *public* randomness beacon: everyone resolving against the same drand round gets the same 32 bytes. If you need per-caller unique randomness, derive it consumer-side: `per_caller = sha256(round_randomness || caller_pubkey)`.

**Why drand and not chain-native randomness (slot hashes, recent_blockhashes)?**
Chain-native sources are grindable or biased by the proposer. drand is threshold-signed by the League of Entropy coalition, so biasing it requires compromising a threshold-sized fraction of the signers — a meaningfully different trust model.

**Why BN254 and not BLS12-381?**
drand supports multiple curves; their `evmnet` chain uses BN254 specifically so Ethereum + Solana (both of which expose BN254 via precompiles/syscalls) can verify cheaply. The `alt_bn128_pairing` syscall is Alea's critical dependency.

**What happens if drand gets compromised?**
drand is threshold-signed: forging a beacon requires compromising more than half the signer set. Compromise of a minority of signers doesn't forge anything. In the catastrophic case where a valid-looking forged beacon does get produced, Alea's verification still rejects anything that fails the pairing check (error 6000) — consumers see a failed transaction, not silent corruption.

**What about front-running?**
drand beacons are public the moment they're published. To prevent front-running in a commit-reveal pattern, consumers enforce `min_resolution_round ≥ current_round + 1` at commit time — the canonical [`example-lottery`](programs/example-lottery/) demonstrates this.

**Can I use Alea on mainnet today?**
Not yet. The program is deployed only to devnet as of v0.1.0. Mainnet deployment gates on external paid audit + Squads multisig transition + the open BPF 6006 runtime test (see CAVEATS). The SDK works today for devnet testing; your mainnet consumer code is ready to go as soon as Phase 5 completes.

**How much does a verify cost?**
Zero protocol fees. Zero oracle fees. You pay Solana's base transaction fee (~5000 lamports = $0.0005 at SOL=$100) plus the compute budget cost (~0.00005 SOL at 900K CU with default priority). No recurring subscription.

**Is Alea production-ready?**
For devnet integration testing: yes. For mainnet production: not yet — an external paid security audit is the canonical gate (see [Status](#status) for the full Phase 5 checklist). The testing evidence shipped with the repo is under [Testing & Validation](#testing--validation).

---

## Prior Art & Credits

- **[randa-mu/bls-solana](https://github.com/randa-mu/bls-solana)** — Randamu (the organization that operates drand) built a BN254 drand verifier prototype for Solana. **Never deployed** to any Solana cluster (verified via RPC across mainnet/devnet/testnet). Alea completes the job; randa-mu taught us the shape of the problem.
- **[kevincharm/bls-bn254](https://github.com/kevincharm/bls-bn254)** — Solidity reference implementation. SVDW algorithm and BN254 constants ported from here + cross-validated against gnark-crypto.
- **[drand / League of Entropy](https://drand.love)** — the coalition of universities, companies, and non-profits that jointly produces the drand randomness beacon Alea verifies.
- **[Paul Miller's noble libraries](https://paulmillr.com/noble/)** — `@noble/curves` + `@noble/hashes` are the JS reference implementations used for test vector generation.
- **arkworks ecosystem** — `ark-ff`, `ark-bn254`, `ark-ec`, `ark-serialize` underpin Alea's field arithmetic.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding conventions, and PR process.

**Solo-maintainer caveat:** Alea is currently solo-maintained and grant-unfunded. Response times for issues and PRs are best-effort. If you need guaranteed SLAs for a commercial integration, open an issue — we'll figure out a path (prioritised support if grant funding is in flight, or a fork-and-maintain recommendation otherwise).

**The `verify` v1 instruction signature is frozen forever.** Additive changes are welcome; breaking changes require a new program ID (new deployment, not an upgrade). CI enforces this on every PR.

---

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE) for third-party attributions.
