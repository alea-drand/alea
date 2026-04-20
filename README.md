# Alea

On-chain drand randomness verification for Solana. No oracle, no callback — one CPI call, 32 bytes of verified randomness.

[![CI](https://github.com/alea-drand/alea/actions/workflows/test.yml/badge.svg)](https://github.com/alea-drand/alea/actions/workflows/test.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/alea-sdk.svg)](https://crates.io/crates/alea-sdk)
[![npm](https://img.shields.io/npm/v/@alea-drand/sdk.svg)](https://www.npmjs.com/package/@alea-drand/sdk)
[![drand](https://img.shields.io/badge/powered%20by-drand-ff6b6b.svg)](https://drand.love)

```rust
use alea_sdk::{self, AleaVerifier};

#[derive(Accounts)]
pub struct ResolveGame<'info> {
    pub alea_program: Program<'info, AleaVerifier>,
    #[account(
        seeds = [b"config"],
        bump,
        seeds::program = alea_program.key(),  // MANDATORY
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,
    pub payer: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
}

pub fn resolve(ctx: Context<ResolveGame>, round: u64, signature: [u8; 64]) -> Result<()> {
    require!(
        alea_sdk::is_round_recent(round, &ctx.accounts.alea_config, &ctx.accounts.clock, 30),
        GameError::StaleBeacon,
    );

    let randomness = alea_sdk::cpi::verify(
        ctx.accounts.alea_program.to_account_info(),
        ctx.accounts.alea_config.to_account_info(),
        ctx.accounts.payer.to_account_info(),
        round,
        signature,
    )?.into_inner();

    // Use randomness immediately. The next CPI overwrites Solana's return slot.
    let winner = u64::from_le_bytes(randomness[0..8].try_into().unwrap()) % 2;
    // ...
    Ok(())
}
```

## Why

Every other randomness option on Solana asks you to trust somebody. ORAO wants you to trust its operator quorum. Switchboard v2 wants you to trust its oracle committee; v3 wants you to trust Intel SGX enclaves. MagicBlock wants you to trust its committee. Pyth Entropy is EVM-only and wants you to trust the Pyth provider anyway. Commit-reveal DIY wants you to trust your own coordinator not to front-run you.

Alea is different because there's no operator to trust. The randomness IS the drand beacon — threshold-signed by Cloudflare, Protocol Labs, EPFL, Kudelski Security, and other members of the League of Entropy — and the BN254 BLS signature is verified on-chain via `alt_bn128_pairing`. If the pairing check passes, the 32 bytes are authentic. The trust surface is drand's threshold signer set. That's it.

drand beacons are public. Everyone resolving against the same round gets the same 32 bytes — a feature for public-draw semantics (lotteries, tournaments, fair launches) and a limitation if you want per-user unique randomness. Derive that yourself with `sha256(round_randomness || user_pubkey)`, or use ORAO — that's what it's for.

## How it works

1. Your program calls `alea_sdk::cpi::verify(program, config, payer, round, signature)`.
2. Alea computes `msg_hash = keccak256(round_be_u64)`.
3. Full SVDW hash-to-curve maps `msg_hash` into a G1 point `M`. This is the hot path at roughly 250–400K CU.
4. Alea calls `alt_bn128_pairing` to check `e(σ, G2_gen) == e(M, pubkey_G2)`. Pairing costs about 48K CU.
5. On success Alea returns `sha256(signature)` as 32 bytes. That matches drand's published `randomness` field byte-for-byte under the `bls-bn254-unchained-on-g1` scheme.

No hinting, no coordinator, no off-chain step. The entire verification happens inside the transaction.

Worst-case compute is about 454K CU, so every transaction calling Alea MUST include `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. The TypeScript SDK injects this automatically. Rust callers building raw transactions have to add it themselves.

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
```

One Config PDA (`seeds = [b"config"]`) holds drand's public key, chain hash, genesis time, and period. No beacon storage, no request/response cycle.

## Mandatory consumer constraints

The constraints look pedantic because they are. Fake-config substitution and beacon replay are both silent compromises of your randomness source — they won't throw an error, they'll just return something an attacker controls.

1. `seeds::program = alea_program.key()` on the `alea_config` account. Without it, an attacker can pass a PDA owned by their own program with attacker-chosen public keys. Anchor does not enforce program-ownership on PDA accounts by default; the account constraint will accept anything matching the seed pattern.

2. `is_round_recent(round, config, clock, max_age_seconds)` before trusting randomness. Alea's `verify` instruction accepts any round, including old ones whose randomness is already public. 30 seconds is a reasonable default; tighten to 3 (one drand round) for adversarial contexts.

3. Capture the return data immediately. Solana's return-data slot is single-use — the next CPI overwrites it. Read the randomness into a local variable before any token transfer or other CPI, or you'll read stale bytes.

`alea_sdk::cpi::verify` also asserts `config.owner == PROGRAM_ID` at the wrapper layer (about 200 CU). That catches non-Anchor callers who bypass the `seeds::program` check — defense-in-depth, not a replacement for the constraint.

The canonical [`programs/example-lottery/`](programs/example-lottery/) demonstrates all three with commit-reveal.

## What it's not for

Three-second drand periods put Alea out of reach for anything needing sub-second response. And because drand beacons are public the moment they publish, Alea is the reveal trigger for sealed-bid auctions, not the private source of the sealed values.

## Status

Devnet is live at [`ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`](https://explorer.solana.com/address/ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U?cluster=devnet). The program is cluster-agnostic — the same vanity ID will deploy to mainnet unchanged. Mainnet deployment is pending; a mainnet Connection fails at the Solana RPC layer ("program not found") until then.

The program is upgradeable by a single deployer keypair today. Migration to a Squads 2-of-3 multisig is planned after mainnet stabilises; the long-term intent is to zero out the upgrade authority once the program has run without critical bugs for a meaningful period. The on-chain `verify` interface is frozen — instruction signature, Config layout, return-data format, and event schema don't change across upgrades. Breaking changes would require a new program ID. CI enforces this via `idl-diff` on every PR.

Alea is solo-maintained and grant-unfunded. Issues and PRs get answered when I see them — usually same-day, not guaranteed. Read [`sdk/rust/CAVEATS.md`](sdk/rust/CAVEATS.md) and [`sdk/typescript/CAVEATS.md`](sdk/typescript/CAVEATS.md) before integrating in production.

## Install

```bash
cargo add alea-sdk
# or: npm install @alea-drand/sdk @solana/web3.js @coral-xyz/anchor
```

Three packages, all v0.1.0: [`alea-verifier`](https://crates.io/crates/alea-verifier) (the on-chain program, importable as a library), [`alea-sdk`](https://crates.io/crates/alea-sdk) (Rust CPI helpers), and [`@alea-drand/sdk`](https://www.npmjs.com/package/@alea-drand/sdk) (TypeScript client for off-chain fetch-and-submit). The full API lives in [`sdk/rust/README.md`](sdk/rust/README.md) and [`sdk/typescript/README.md`](sdk/typescript/README.md).

Devnet addresses: Config PDA is `6anALRxD98Tw7zbA9d5i4NJfTvxDsNBHohHVJWxv2Xm8`; upgrade authority is `9cPWdtoR7sW7VVYxfrJZ9ekxX2fZctskXn3L4BSmafcc`; the deployed binary's SHA256 is `8965062489fdcdbb538597545fc6692f3f580d770d34f2d42000a70560984b1c`.

Error codes are canonical in [`programs/alea-verifier/src/errors.rs`](programs/alea-verifier/src/errors.rs). CI's `idl-diff` check prevents silent schema drift. Codes 6000–6012 are on-chain; 6100–6103 are TypeScript-SDK-side (network failures and input validation).

## Security

Report vulnerabilities via [GitHub Security Advisory](https://github.com/alea-drand/alea/security/advisories/new). Scope and disclosure timeline in [`.github/SECURITY.md`](.github/SECURITY.md). Alea holds no user funds, so the attack surface is binary replacement of the on-chain program — consumers wanting belt-and-suspenders protection can pin against the published binary SHA256 above and refuse to transact if the deployed binary changes unexpectedly.

70+ Rust and 37+ TypeScript tests run on every PR. 23 billion cargo-fuzz iterations across the cryptographic pipeline, zero crashes — proof tarballs on [the latest release](https://github.com/alea-drand/alea/releases). Supply chain covered by `cargo-deny`, `npm audit`, and `trufflehog` secret-scan on every PR plus a weekly cron.

## Credits

[randa-mu/bls-solana](https://github.com/randa-mu/bls-solana) — Randamu, the organization that operates drand, built a BN254 verifier prototype for Solana. It was never deployed to any cluster. Alea completes the work; randa-mu defined the shape of the problem. [kevincharm/bls-bn254](https://github.com/kevincharm/bls-bn254) is the Solidity reference — SVDW and BN254 constants are ported from there, cross-validated against gnark-crypto. [drand and the League of Entropy](https://drand.love) produce the beacon Alea verifies. [Paul Miller's noble libraries](https://paulmillr.com/noble/) generated test vectors. The [arkworks ecosystem](https://arkworks.rs) underpins the field arithmetic.

## License & Contributing

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE). Development notes in [CONTRIBUTING.md](CONTRIBUTING.md).
