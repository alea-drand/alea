import {
  DRAND_CHAIN_HASH,
  DRAND_GENESIS_TIME,
  DRAND_PERIOD,
  DRAND_ENDPOINTS,
} from "./constants.js";
import { AleaError } from "./errors.js";

export interface DrandBeacon {
  round: bigint;
  signature: Uint8Array;
  /** hex string — NOT on-chain verified; use getVerifiedRandomness() for trust */
  unverifiedRandomness: string;
}

// T1.09 — bigint canonical. Year-2086 safety.
export function getCurrentRound(): bigint {
  const now = BigInt(Math.floor(Date.now() / 1000));
  const genesis = BigInt(DRAND_GENESIS_TIME);
  const period = BigInt(DRAND_PERIOD);
  return (now - genesis) / period + 1n;
}

export function getRoundAt(timestamp: bigint): bigint {
  const genesis = BigInt(DRAND_GENESIS_TIME);
  const period = BigInt(DRAND_PERIOD);
  return (timestamp - genesis) / period + 1n;
}

// T1.19 — symmetric with Rust CPI is_round_recent. Pure function, no I/O.
// Behavior: future rounds (roundTs > clock.unixTimestamp) return TRUE, matching
// Rust's saturating_sub semantics (current - round = 0 when future → 0 <= max).
// Rationale: a consumer verifying right at the edge of round emission shouldn't
// spuriously fail due to sub-second clock skew; the on-chain Rust check will
// accept the same round a moment later anyway. Aligned 2026-04-19 (phase 4.5).
export function isRoundRecent(
  round: bigint,
  config: { genesisTime: bigint; period: bigint },
  clock: { unixTimestamp: bigint },
  maxAgeSeconds: bigint,
): boolean {
  if (round === 0n) return false;
  const roundTs = config.genesisTime + (round - 1n) * config.period;
  const age =
    clock.unixTimestamp > roundTs ? clock.unixTimestamp - roundTs : 0n;
  return age <= maxAgeSeconds;
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

// T2.01 — 3 retries × 5 endpoints × 5s timeout + 1s inter-attempt delay = 77s worst case.
export async function fetchBeacon(round?: bigint): Promise<DrandBeacon> {
  const MAX_RETRIES = 3;
  const RETRY_DELAY_MS = 1000;
  const TIMEOUT_MS = 5000;

  let targetRound: bigint = round ?? getCurrentRound();
  const chainHash = DRAND_CHAIN_HASH;

  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    for (const endpoint of DRAND_ENDPOINTS) {
      try {
        const url = `${endpoint}/${chainHash}/public/${targetRound.toString()}`;
        const response = await fetch(url, {
          signal: AbortSignal.timeout(TIMEOUT_MS),
        });
        if (response.ok) {
          const data = (await response.json()) as {
            round: number;
            signature: string;
            randomness: string;
          };
          return {
            round: BigInt(data.round),
            signature: hexToBytes(data.signature),
            unverifiedRandomness: data.randomness,
          };
        }
        if (response.status === 404 && round === undefined) {
          // Round not yet produced — back off by one and restart endpoint loop
          targetRound = targetRound > 1n ? targetRound - 1n : 1n;
          break;
        }
      } catch {
        // timeout or network error — try next endpoint
        continue;
      }
    }
    if (attempt < MAX_RETRIES - 1) {
      await new Promise<void>((r) => setTimeout(r, RETRY_DELAY_MS));
    }
  }

  throw new AleaError(0, "All drand endpoints failed after retries");
}
