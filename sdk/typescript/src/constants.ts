import { PublicKey } from "@solana/web3.js";

export const DRAND_CHAIN_HASH =
  "04f1e9062b8a81f848fded9c12306733282b2727ecced50032187751166ec8c3";

export const DRAND_GENESIS_TIME = 1727521075;

export const DRAND_PERIOD = 3;

export const DRAND_ENDPOINTS: readonly string[] = [
  "https://api.drand.sh",
  "https://api2.drand.sh",
  "https://api3.drand.sh",
  "https://drand.cloudflare.com",
  "https://api.drand.secureweb3.com:6875",
];

// Alea's program ID is CLUSTER-AGNOSTIC: the same vanity ID deploys
// identically to devnet, mainnet, localnet, or any Solana cluster you
// choose to run on. Your Connection object determines which cluster's
// deployment the tx actually lands against.
//
// As of v0.1.0: Alea is live on DEVNET only. Mainnet deploy is the
// Phase 5 gate. If you point a mainnet Connection at the SDK today,
// the tx will fail at the Solana RPC layer with "Program not found"
// (or similar) — Solana itself is the safety rail, not the SDK.
//
// DEVNET_PROGRAM_ID and MAINNET_PROGRAM_ID are exposed as distinct
// symbols for clarity-of-intent in consumer code (`programId: MAINNET_
// PROGRAM_ID` reads as 'I intend mainnet'), but they point to the same
// bytes. See ADR 0028 + sdk/typescript/CAVEATS.md §1 for cluster state.
export const DEVNET_PROGRAM_ID = new PublicKey(
  "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U",
);

export const MAINNET_PROGRAM_ID = DEVNET_PROGRAM_ID;
