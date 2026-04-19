# Alea

> **The die is cast.** On-chain [drand](https://drand.love) BN254 verifier for Solana.

[![CI](https://github.com/alea-drand/alea/actions/workflows/test.yml/badge.svg)](https://github.com/alea-drand/alea/actions/workflows/test.yml)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/alea-sdk.svg)](https://crates.io/crates/alea-sdk)
[![npm](https://img.shields.io/npm/v/@alea/sdk.svg)](https://www.npmjs.com/package/@alea/sdk)
[![drand](https://img.shields.io/badge/powered%20by-drand-ff6b6b.svg)](https://drand.love)

Alea is the **first production drand BN254 verifier on Solana**¹. Any Solana program can call `alea_sdk::cpi::verify(program, config, payer, round, signature)` and get 32 bytes of cryptographically verified randomness in a single transaction — no callbacks, no keepers, no off-chain coordinators, no tokens, no fees.

**It is a free, Apache 2.0 licensed public good. No commercial version is planned.** See [ADR 0018](build-spec/decisions/0018-monetization.md).

¹ "First production" framing per pre-submission RPC verification ([T1.15](build-spec/phases/phase-8-outreach.md) runs 24h before hackathon). Prior art [randa-mu/bls-solana](https://github.com/randa-mu/bls-solana) is credited ([ADR 0024](build-spec/decisions/0024-prior-art.md)) — they built a working prototype but never deployed to any Solana cluster.

## Why Alea

| Existing option | Cost | Friction | Provenance |
|-----------------|------|----------|------------|
| [ORAO VRF](https://orao.network) | ~$0.15 / verification | Request-then-callback (2 tx) | Single-operator trust |
| [Switchboard](https://switchboard.xyz) randomness | ~$0.01+ / verification | Oracle round trip | Oracle committee trust |
| Commit-reveal (DIY) | 0 | 2 tx + coordinator logic | No cryptographic provenance |
| **Alea** | **$0** (user pays ~0.000005 SOL for Solana tx) | **1 CPI call, single tx** | **drand threshold sig — 23 orgs including Cloudflare, Protocol Labs** |

## Quick Start

```bash
cargo add alea-sdk
# or: npm install @alea/sdk
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
        seeds::program = alea_program.key(),  // MANDATORY — see ADR 0034
    )]
    pub alea_config: Account<'info, alea_sdk::Config>,
    pub payer: Signer<'info>,
    pub clock: Sysvar<'info, Clock>,
    // ... your accounts ...
}

pub fn resolve_game(ctx: Context<ResolveGame>, round: u64, signature: [u8; 64]) -> Result<()> {
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
    )?;

    // Use the 32-byte randomness for game resolution
    let winner_index = u64::from_le_bytes(randomness[0..8].try_into().unwrap()) % 2;
    // ... settle logic ...

    Ok(())
}
```

### TypeScript / Node (off-chain fetch + submit)

```typescript
import { getVerifiedRandomness } from "@alea/sdk";
import { Connection, Keypair } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const signer = Keypair.fromSecretKey(/* your keypair bytes */);

// One-liner: fetches the latest drand beacon, submits verify tx, returns 32 bytes.
// v0.1.x ships DEVNET as the default program ID; mainnet is a throw-proxy until
// Phase 5 (see sdk/typescript/README.md "Devnet vs Mainnet" for the swap).
const randomness: Uint8Array = await getVerifiedRandomness({
  connection,
  signer,
  // round defaults to latest drand round — pass a bigint to pin a specific one
});

console.log("Verified randomness (hex):", Buffer.from(randomness).toString("hex"));
```

Full integration guide: [docs site](https://alea.so) (live post-mainnet) + in-repo SDK READMEs under `sdk/typescript/` and `sdk/rust/`.

## How It Works

1. Your program calls `alea_sdk::cpi::verify(program, config, payer, round, signature)` with a drand beacon (canonical 5-arg signature per ADR 0028)
2. Alea computes `msg_hash = keccak256(round_be_u64)` and runs full SVDW hash-to-curve on-chain (~250-400K CU)
3. Alea invokes Solana's `alt_bn128_pairing` syscall to verify `e(σ, G2_gen) == e(M, pubkey_G2)` (~48K CU)
4. On success, Alea returns `sha256(signature_bytes)` — matches drand's published `randomness` field byte-for-byte (see [ADR 0036](build-spec/decisions/0036-randomness-derivation-sha256.md))

No off-chain steps. No hinting. No trusted coordinator. Pure cryptographic verification.

## Architecture

```
drand Network (23 orgs) → @alea/sdk (fetches beacon) → Solana tx
                                                          ↓
                                                 alea-verifier (on-chain)
                                                   1. keccak256(round)
                                                   2. SVDW hash-to-G1 [~250K CU]
                                                   3. BLS pairing check [~49K CU]
                                                   4. sha256(signature) → randomness
                                                          ↓
                                             Consumer programs (via CPI)
```

One on-chain Config PDA holds drand's G2 public key, chain_hash, and period. Fully stateless verification otherwise — no beacon storage, no request/response cycle.

## Governance & Upgrade Roadmap

| Phase | Authority | Trigger |
|-------|-----------|---------|
| v1 (mainnet launch) | Deployer keypair (Aaron) | Initial release |
| v2 | Squads 2-of-3 multisig (Aaron + Randamu contact + SF contact) | **90 days after mainnet** OR $50K TVL OR first audit (whichever first) |
| v3 | Immutable | Post-audit + 6+ months stable operation |

The v2 multisig transition is a **public commitment**. Failure to execute within 90 days of mainnet deploy is a trust-breaking event, explicitly flagged in [ADR 0009](build-spec/decisions/0009-upgrade-authority.md).

## Security

- **Report vulnerabilities:** [GitHub Security Advisory](https://github.com/alea-drand/alea/security/advisories/new) (primary) or `security@alea.so` (fallback — self-hosted Stalwart Mail on VPS, inbound-only). Response SLAs: 72h ack, 7d triage, 30d P0 fix (see SECURITY.md for pre-multisig SLA exception). See [`.github/SECURITY.md`](.github/SECURITY.md).
- **Threat model:** [`build-spec/architecture/security-model.md`](build-spec/architecture/security-model.md) — 7 threats documented including fake-config substitution (ADR 0034), drand key rotation playbook (T2.26), and the deployer keypair loss scenario (T2.25).
- **Bug bounty intent:** post-grant activation. See [`.github/SECURITY.md`](.github/SECURITY.md) §"Bug Bounty (Intent)".

## Maintenance Tiers ([ADR 0032](build-spec/decisions/0032-maintenance-tiers.md))

- **Tier A (default, no grant):** 3 months active support post-mainnet, best-effort bug fixes, multisig transition, never-abandon baseline.
- **Tier B (grant-activated, $15K-$75K):** 6-12 months full-time BD, integration hackathons, community SDK bindings, formal audit.

If you need guaranteed SLAs for a commercial integration, open an issue — we'll figure out a path.

## Packages

v0.1.0 publishes `alea-sdk` to crates.io and `@alea/sdk` to npm after devnet is stable. The on-chain `alea-verifier` program ID is published in CHANGELOG.md once deployed (devnet first, then mainnet post-Phase-5 per the governance roadmap below).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Solo-maintainer caveat: Tier A response times are best-effort until Tier B activates.

The `verify` v1 instruction signature is **frozen forever** per [ADR 0028](build-spec/decisions/0028-cpi-versioning.md). Additive-only changes are welcome; breaking changes require a new program ID (new deployment, not an upgrade).

## Prior Art & Credits

- **[randa-mu/bls-solana](https://github.com/randa-mu/bls-solana)** — Randamu (drand stewards) built a BN254 drand verifier prototype for Solana. Never deployed (verified via RPC against mainnet/devnet/testnet). Alea completes the job; randa-mu taught us the shape of the problem.
- **[kevincharm/bls-bn254](https://github.com/kevincharm/bls-bn254)** — Solidity reference implementation. SVDW algorithm and BN254 constants ported from here.
- **[drand / League of Entropy](https://drand.love)** — 23 organizations including Cloudflare, Protocol Labs, Ethereum Foundation, who produce the randomness Alea verifies.
- **[Paul Miller's noble libraries](https://paulmillr.com/noble/)** — `@noble/curves` + `@noble/hashes` are the JS reference implementations used for test vector generation.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE) for third-party attributions.

---

*"alea iacta est" — Julius Caesar at the Rubicon, 49 BC. [Alea](https://en.wikipedia.org/wiki/Alea_iacta_est) = Latin for "die" (the plural is "aleae"). The die is cast, the randomness is on-chain.*
