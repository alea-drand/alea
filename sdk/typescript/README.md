# @alea/sdk

Verified drand randomness on Solana in one call.

> **Read [CAVEATS.md](CAVEATS.md) before using in production.** This SDK is pre-mainnet (devnet only as of v0.1.0). An external audit is scheduled for Phase 5.

## Install

```bash
npm install @alea/sdk
```

## Quick Start — Browser (User Pays)

```typescript
import { getVerifiedRandomness } from "@alea/sdk";
import { useWallet } from "@solana/wallet-adapter-react";
import { Connection } from "@solana/web3.js";

const connection = new Connection("https://api.devnet.solana.com");
const wallet = useWallet(); // from @solana/wallet-adapter-react

// Returns 32 bytes of verified randomness. User approves a wallet popup.
const randomness = await getVerifiedRandomness({
  connection,
  signer: wallet,
});
console.log(`Randomness: ${Buffer.from(randomness).toString("hex")}`);
```

## Quick Start — Server (Developer Pays)

```typescript
import { getVerifiedRandomness } from "@alea/sdk";
import { Keypair, Connection } from "@solana/web3.js";

const keypair = Keypair.fromSecretKey(/* your key bytes */);
const connection = new Connection("https://api.devnet.solana.com");

const randomness = await getVerifiedRandomness({
  connection,
  signer: keypair,
});
```

## API Reference

### `getVerifiedRandomness(options)`

High-level entry point. Fetches a drand beacon, submits a Solana transaction, and returns 32 bytes of verified randomness.

```typescript
async function getVerifiedRandomness(options: {
  connection: Connection;
  signer: Keypair | Wallet;
  programId?: PublicKey;     // defaults to DEVNET_PROGRAM_ID
  commitment?: Commitment;   // defaults to "confirmed"
  round?: bigint;            // defaults to latest available round
  computeUnits?: number;     // defaults to 900_000
}): Promise<Uint8Array>      // 32 bytes
```

### `verifyDrandBeacon(args)`

Canonical IDL-based instruction builder. Use when you have a specific round + signature already fetched.

```typescript
async function verifyDrandBeacon(args: {
  connection: Connection;
  signer: Keypair | Wallet;
  round: bigint;
  signature: Uint8Array;   // 64-byte G1 point
  programId?: PublicKey;
  computeUnits?: number;
}): Promise<Uint8Array>    // 32 bytes
```

### `fetchBeacon(round?)`

Fetches a drand beacon without Solana interaction. Uses 5-endpoint fallback with 3 retries.

> **WARNING:** Returns UNVERIFIED data from the drand API. Use `getVerifiedRandomness()` for trustless randomness.

```typescript
async function fetchBeacon(round?: bigint): Promise<{
  round: bigint;
  signature: Uint8Array;
  unverifiedRandomness: string;  // hex — NOT on-chain verified
}>
```

### `getCurrentRound()`

Returns the latest drand round number as a `bigint`.

### `getRoundAt(timestamp: bigint)`

Returns the drand round number for a given Unix timestamp (seconds).

### `isRoundRecent(round, config, clock, maxAgeSeconds)`

Pure function — symmetric with Rust CPI `is_round_recent`. Call before `getVerifiedRandomness` to cheaply reject obvious replay attempts.

```typescript
function isRoundRecent(
  round: bigint,
  config: { genesisTime: bigint; period: bigint },
  clock: { unixTimestamp: bigint },
  maxAgeSeconds: bigint,
): boolean
```

### `createVerifyInstruction(options)`

Raw instruction builder for advanced use. Use `verifyDrandBeacon` for auto-wired Anchor IDL path.

```typescript
function createVerifyInstruction(options: {
  round: bigint;
  signature: Uint8Array;
  programId?: PublicKey;
}): TransactionInstruction
```

### `getConfigAddress(programId?)`

Derives the Alea config PDA address.

### Constants

```typescript
DRAND_CHAIN_HASH    // evmnet chain hash (hex)
DRAND_GENESIS_TIME  // 1727521075 (Unix seconds)
DRAND_PERIOD        // 3 (seconds)
DRAND_ENDPOINTS     // 5 API endpoints with fallback ordering
DEVNET_PROGRAM_ID   // ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U
MAINNET_PROGRAM_ID  // throws — not set until Phase 5 mainnet deploy
```

### `AleaError`

```typescript
class AleaError extends Error {
  code: number;
}
```

### `ERRORS`

Error code map (6000–6009 + Anchor 2001):

| Code | Name | Description |
|------|------|-------------|
| 2001 | ConstraintHasOne | Signer is not the config authority |
| 6000 | InvalidSignature | BLS signature verification failed |
| 6001 | InvalidG1Point | Signature bytes are not a valid G1 point |
| 6002 | RoundZero | Round number must be greater than 0 |
| 6003 | InvalidFieldElement | Field element out of valid range |
| 6004 | NoSquareRoot | Square root does not exist |
| 6005 | InvalidG2Point | Public key bytes are not a valid G2 point |
| 6006 | PairingError | alt_bn128_pairing syscall failed |
| 6007 | WrongChainHash | chain_hash does not match expected |
| 6008 | WrongPubkey | pubkey_g2 does not match expected |
| 6009 | ReturnDataMissing | CPI consumer received no return data |

## Consumer Responsibility

This SDK is for off-chain consumers. If you're building an on-chain program that CPIs to Alea, see the `alea-sdk` Rust crate's Security section for mandatory `is_round_recent` constraints that prevent replay attacks.

## Program IDs

| Network | Program ID |
|---------|-----------|
| Devnet | `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` |
| Mainnet | Pending Phase 5 — pass `{ programId }` explicitly |

## License

Apache 2.0 — see [LICENSE](LICENSE)

[GitHub: alea-drand/alea](https://github.com/alea-drand/alea)
