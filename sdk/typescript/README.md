# @alea-drand/sdk

Verified drand randomness on Solana in one call.

> **v0.1.x is DEVNET only — mainnet deployment pending.** Alea's program ID is cluster-agnostic; `DEVNET_PROGRAM_ID` and `MAINNET_PROGRAM_ID` point to the same bytes. Using a mainnet `Connection` before mainnet deploys fails at the Solana RPC layer ("program not found"). Read [CAVEATS.md](CAVEATS.md) before production use.

## Install

```bash
npm install @alea-drand/sdk @solana/web3.js @coral-xyz/anchor
```

`@solana/web3.js` and `@coral-xyz/anchor` are **peer dependencies** — consumers install them directly so your bundle has one copy of each (prevents class-identity issues with `PublicKey` instances across mismatched versions).

Requires **Node 18+** and **ESM**. This package is ESM-only; use `import`, not `require()`. Works in browsers with any modern bundler (Vite, webpack 5, Next.js App Router, esbuild) — no polyfills needed.

## Quick Start — Browser (user pays, via wallet-adapter)

```typescript
import { getVerifiedRandomness } from "@alea-drand/sdk";
import { useWallet } from "@solana/wallet-adapter-react";
import { useConnection } from "@solana/wallet-adapter-react";

function RandomnessButton() {
  const { connection } = useConnection();
  const wallet = useWallet(); // WalletContextState

  async function draw() {
    // User approves a single popup. Returns 32 bytes of verified randomness.
    const randomness = await getVerifiedRandomness({
      connection,
      signer: wallet,
    });
    console.log(`Randomness: ${Buffer.from(randomness).toString("hex")}`);
  }

  return <button onClick={draw}>Draw</button>;
}
```

## Quick Start — Server (developer pays, via Keypair)

```typescript
import { getVerifiedRandomness } from "@alea-drand/sdk";
import { Keypair, Connection } from "@solana/web3.js";
import { readFileSync } from "node:fs";

const keypair = Keypair.fromSecretKey(
  new Uint8Array(JSON.parse(readFileSync(process.env.KEYPAIR_PATH!, "utf8"))),
);
const connection = new Connection("https://api.devnet.solana.com", "confirmed");

const randomness = await getVerifiedRandomness({
  connection,
  signer: keypair,
});
console.log(Buffer.from(randomness).toString("hex"));
```

## Testing on Devnet

Before integrating into prod, run the SDK against live devnet:

```bash
# 1. Install the SDK
npm install @alea-drand/sdk @solana/web3.js @coral-xyz/anchor

# 2. Generate or load a Solana devnet keypair
solana-keygen new --outfile ~/.config/solana/alea-test.json

# 3. Fund it with devnet SOL
solana airdrop 1 ~/.config/solana/alea-test.json --url devnet
# Backup faucet if solana-labs is dry: 76 Devs Discord / LamportDAO

# 4. Run the quick-start above against devnet
# Expected: 32 bytes of randomness (hex) printed in under 10 seconds
```

Live devnet program: [`ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`](https://explorer.solana.com/address/ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U?cluster=devnet).

## API Reference

### `getVerifiedRandomness(options)`

High-level entry point. Fetches a drand beacon, submits a Solana transaction, and returns 32 bytes of verified randomness.

```typescript
async function getVerifiedRandomness(options: {
  connection: Connection;
  signer: Keypair | Wallet;         // Keypair (Node) or wallet-adapter WalletContextState
  programId?: PublicKey;            // defaults to DEVNET_PROGRAM_ID
  round?: bigint;                   // defaults to latest available round
  computeUnits?: number;            // defaults to 900_000
}): Promise<Uint8Array>             // 32 bytes
```

### `verifyDrandBeacon(args)`

Lower-level IDL-based submission. Use when you have a round + signature fetched out-of-band.

```typescript
async function verifyDrandBeacon(args: {
  connection: Connection;
  signer: Keypair | Wallet;
  round: bigint;                    // [1, 2^64-1]
  signature: Uint8Array;            // exactly 64 bytes, G1 uncompressed x||y
  programId?: PublicKey;
  computeUnits?: number;
}): Promise<Uint8Array>             // 32 bytes
```

### `verifyDrandBeaconWithMeta(args)` — since 0.2.0

Same as `verifyDrandBeacon` but returns the full tx metadata alongside the randomness. Use when you need to link to Explorer, report compute units / fees, or surface the slot to end users.

```typescript
type VerifyMeta = {
  randomness: Uint8Array;           // 32 bytes
  tx: string;                       // base58 Solana tx signature
  slot: number;                     // confirmed-commitment slot
  computeUnitsUsed: number;         // from tx meta.computeUnitsConsumed
  costLamports: number;             // from tx meta.fee
};

async function verifyDrandBeaconWithMeta(args: {
  connection: Connection;
  signer: Keypair | Wallet;
  round: bigint;
  signature: Uint8Array;
  programId?: PublicKey;
  computeUnits?: number;
  signal?: AbortSignal;
  skipPreflight?: boolean;
}): Promise<VerifyMeta>
```

### `getVerifiedRandomnessWithMeta(options)` — since 0.2.0

One-shot variant of `getVerifiedRandomness` that returns the drand round + drand signature + full on-chain meta in a single call. Used by the [alea.so](https://alea.so) public relayer; useful anywhere you want to display end-to-end provenance.

```typescript
async function getVerifiedRandomnessWithMeta(options: {
  connection: Connection;
  signer: Keypair | Wallet;
  programId?: PublicKey;
  round?: bigint;
  computeUnits?: number;
  signal?: AbortSignal;
  skipPreflight?: boolean;
}): Promise<VerifyMeta & {
  round: bigint;                    // drand round that was verified
  signature: Uint8Array;            // drand sig (G1, 64 bytes)
}>
```

### `fetchBeacon(round?)`

Fetches a drand beacon without Solana interaction. Uses 5-endpoint fallback with 3 retries, validates returned round matches requested.

> **WARNING:** Returns UNVERIFIED data from the drand API. Use `getVerifiedRandomness()` for trustless randomness.

```typescript
async function fetchBeacon(round?: bigint): Promise<{
  round: bigint;
  signature: Uint8Array;
  unverifiedRandomness: string;  // hex — NOT on-chain verified
}>
```

### `getCurrentRound()` / `getRoundAt(timestamp)`

Compute drand round numbers (canonical `bigint`). `getCurrentRound` uses `Date.now()`.

### `isRoundRecent(round, config, clock, maxAgeSeconds)`

Pure function — symmetric with Rust CPI `is_round_recent`. Call before `getVerifiedRandomness` to cheaply reject obvious replay attempts.

Future rounds (`roundTs > clock.unixTimestamp`) return `true` — this matches Rust's saturating-sub semantics so the TS and Rust checks agree at the round-emission edge.

```typescript
function isRoundRecent(
  round: bigint,
  config: { genesisTime: bigint; period: bigint },
  clock: { unixTimestamp: bigint },
  maxAgeSeconds: bigint,
): boolean
```

### `createVerifyInstruction(options)`

Raw instruction builder for advanced use (multi-ix composition, versioned transactions). Use `verifyDrandBeacon` for the common path.

```typescript
function createVerifyInstruction(options: {
  round: bigint;
  signature: Uint8Array;
  payer: PublicKey;                 // signer — included in keys automatically
  programId?: PublicKey;
}): TransactionInstruction
```

### `getConfigAddress(programId?)`

Derives the Alea Config PDA address (seeds `[b"config"]`).

### Constants

```typescript
DRAND_CHAIN_HASH    // evmnet chain hash (hex, 64 chars)
DRAND_GENESIS_TIME  // 1727521075 (Unix seconds)
DRAND_PERIOD        // 3 (seconds per drand round)
DRAND_ENDPOINTS     // 5 fallback URLs (can be replaced for custom trust)
DEVNET_PROGRAM_ID   // ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U
MAINNET_PROGRAM_ID  // === DEVNET_PROGRAM_ID; cluster comes from your Connection
```

### `AleaError`

```typescript
class AleaError extends Error {
  readonly code: number;
}
```

### Error Codes

On-chain errors (from `alea-verifier`) + SDK-side errors (6100+):

| Code | Name | Meaning |
|------|------|---------|
| 2001 | `ConstraintHasOne` | Signer is not the config authority (Anchor framework) |
| 3010 | `AccountNotSigner` | Account passed without signature (Anchor framework) |
| 6000 | `InvalidSignature` | BLS pairing check failed — wrong sig for this round |
| 6001 | `InvalidG1Point` | Signature bytes are not on the BN254 G1 curve |
| 6002 | `RoundZero` | Round must be > 0 (drand genesis sentinel) |
| 6003 | `InvalidFieldElement` | **Reserved (unreachable in v1)** |
| 6004 | `NoSquareRoot` | SVDW exhausted candidates (infrastructure failure) |
| 6005 | `InvalidG2Point` | **Reserved (unreachable)** |
| 6006 | `PairingError` | alt_bn128_pairing syscall failed (infrastructure) |
| 6007 | `WrongChainHash` | `Config.chain_hash` does not match evmnet |
| 6008 | `WrongPubkey` | `Config.pubkey_g2` does not match evmnet (or `alea-sdk` owner check) |
| 6009 | `ReturnDataMissing` | **Reserved (unreachable under ADR 0030)** |
| 6010 | `InvalidGenesisTime` | `Config.genesis_time` mismatch |
| 6011 | `InvalidPeriod` | `Config.period` mismatch |
| 6012 | `UnauthorizedInit` | initialize signer is not the upgrade authority |
| **6100** | `DrandFetchFailed` | **SDK** — all drand endpoints failed after retries |
| **6101** | `DrandRoundMismatch` | **SDK** — endpoint returned a different round than requested (possible compromise) |
| **6102** | `InvalidInput` | **SDK** — input validation failed at SDK boundary |

The `ERRORS` map is exported and frozen at module load (`Object.freeze` + `Readonly<Record>`).

## Consumer Responsibility

This SDK is for off-chain consumers. If you're building an on-chain program that CPIs to Alea, see the [`alea-sdk` Rust crate README](https://crates.io/crates/alea-sdk) for the **mandatory** `seeds::program` + `is_round_recent` constraints. Omitting either ships an exploitable program.

## Program IDs

| Network | Program ID |
|---------|-----------|
| Devnet  | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` |
| Mainnet | Same vanity ID; program not yet deployed on mainnet. Mainnet `Connection` will fail at the RPC layer with "program not found" until deploy. |

## Zero Telemetry

`@alea-drand/sdk` sends **no analytics, no telemetry, no phone-home**. Your only network calls are:
- Drand API endpoints (for beacon fetch, 5 URLs with fallback — fully replaceable via `DRAND_ENDPOINTS` override)
- Solana RPC endpoint of your choice (passed in as `connection`)

Nothing else. Verify by inspecting [`src/client.ts`](https://github.com/alea-drand/alea/blob/main/sdk/typescript/client.ts) and [`src/drand.ts`](https://github.com/alea-drand/alea/blob/main/sdk/typescript/drand.ts).

## How to Verify This Package

Every release carries an [npm provenance attestation](https://docs.npmjs.com/generating-provenance-statements) signed by Sigstore + the GitHub Actions publish workflow. Confirm before use:

```bash
npm audit signatures
```

Or visit [`@alea-drand/sdk` on npm](https://www.npmjs.com/package/@alea-drand/sdk) and look for the green "Provenance" badge. No badge → do not install.

## Community & Support

- **Bugs / feature requests:** [GitHub Issues](https://github.com/alea-drand/alea/issues)
- **Questions / integrations:** [GitHub Discussions](https://github.com/alea-drand/alea/discussions)
- **Security reports:** [GitHub Security Advisory](https://github.com/alea-drand/alea/security/advisories/new) (private)
- **GitHub:** [alea-drand/alea](https://github.com/alea-drand/alea)

## License

Apache 2.0 — see [LICENSE](LICENSE).
