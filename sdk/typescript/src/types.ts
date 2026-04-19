import type { PublicKey } from "@solana/web3.js";

export interface DrandConfig {
  genesisTime: bigint;
  period: bigint;
}

export interface SolanaClock {
  unixTimestamp: bigint;
}

export interface BeaconResult {
  round: bigint;
  /** Raw G1 point bytes (64 bytes) */
  signature: Uint8Array;
  /** hex — NOT on-chain verified */
  unverifiedRandomness: string;
}

export interface VerifyOptions {
  programId?: PublicKey;
  computeUnits?: number;
}
