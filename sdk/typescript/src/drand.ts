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

// Phase 4.5 T1-03 — validate hex input before conversion. A drand G1
// signature is exactly 128 hex chars (64 bytes, uncompressed x||y).
// Previous implementation silently zero-filled invalid chars via
// parseInt(NaN) and truncated odd-length strings — a MITM drand endpoint
// could push all-zeros signatures, corrupt randomness, or silent wrong-
// bytes. The validation below throws early with a readable AleaError.
function hexToBytes(hex: string): Uint8Array {
  if (typeof hex !== "string") {
    throw new AleaError(6102, `InvalidInput: hex must be string (got ${typeof hex})`);
  }
  if (hex.length !== 128) {
    throw new AleaError(
      6102,
      `InvalidInput: drand signature hex must be exactly 128 chars (got ${hex.length})`,
    );
  }
  if (!/^[0-9a-f]+$/i.test(hex)) {
    throw new AleaError(
      6102,
      "InvalidInput: drand signature hex contains non-hex characters",
    );
  }
  const bytes = new Uint8Array(64);
  for (let i = 0; i < 128; i += 2) {
    bytes[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return bytes;
}

// T2.01 — 3 retries × 5 endpoints × 5s timeout + 1s inter-attempt delay = 77s worst case.
//
// Phase 4.5 hardening:
// - T1-02: verify returned `data.round` matches requested target (compromised
//   endpoint cannot serve a valid sig for a different round without detection)
// - T2-05: cap response body via Content-Length (drand beacons are ~200B;
//   reject anything over 4KB to prevent OOM attacks)
// - T2-06: `redirect: "error"` — no following redirects (MITM prevention;
//   a compromised CDN cannot redirect to an attacker-controlled domain)
// - T2-07: exhaustion throws 6100 (was code 0 which wasn't in ERRORS map)
// - T2-15: accept caller AbortSignal to cancel mid-retry-loop (e.g., user
//   navigates away mid-page — drop the 77s worst-case hang cleanly)
export async function fetchBeacon(
  round?: bigint,
  opts?: { signal?: AbortSignal },
): Promise<DrandBeacon> {
  const MAX_RETRIES = 3;
  const RETRY_DELAY_MS = 1000;
  const TIMEOUT_MS = 5000;
  const MAX_RESPONSE_BYTES = 4096; // drand beacons are ~200B; anything over 4KB is suspicious

  let targetRound: bigint = round ?? getCurrentRound();
  const chainHash = DRAND_CHAIN_HASH;
  const userSignal = opts?.signal;

  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    // T2-15 abort checkpoint — fail fast if caller cancelled.
    if (userSignal?.aborted) {
      throw new AleaError(6103, "fetchBeacon aborted by caller");
    }
    for (const endpoint of DRAND_ENDPOINTS) {
      try {
        const url = `${endpoint}/${chainHash}/public/${targetRound.toString()}`;
        // Compose user abort with per-request timeout. AbortSignal.any
        // requires Node 20+ / modern browsers; falls back to timeout-only
        // if unavailable (older Node 18 early-patch). userSignal still
        // checked at loop boundaries above.
        const perReqTimeout = AbortSignal.timeout(TIMEOUT_MS);
        const signal =
          userSignal && typeof (AbortSignal as unknown as { any?: Function }).any === "function"
            ? (AbortSignal as unknown as { any: (s: AbortSignal[]) => AbortSignal }).any([
                perReqTimeout,
                userSignal,
              ])
            : perReqTimeout;
        const response = await fetch(url, {
          signal,
          redirect: "error",
        });
        if (response.ok) {
          // T2-05: guard body size via Content-Length when present
          const contentLength = response.headers.get("content-length");
          if (contentLength !== null) {
            const len = parseInt(contentLength, 10);
            if (Number.isFinite(len) && len > MAX_RESPONSE_BYTES) {
              // Oversized response from this endpoint — skip, don't parse.
              continue;
            }
          }
          const data = (await response.json()) as {
            round: number;
            signature: string;
            randomness: string;
          };
          // T1-02: endpoint must confirm it served the round we asked for.
          // A compromised endpoint could return a validly-signed older
          // beacon (different round); the BLS pairing check on-chain would
          // accept it — consumers with wider `is_round_recent` windows than
          // one drand period (3s) could then take attacker-chosen randomness.
          // Rejecting the mismatch here forces the SDK to fall through to
          // the next endpoint, defeating the substitution attack.
          if (BigInt(data.round) !== targetRound) {
            continue;
          }
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
        // timeout, network error, hexToBytes AleaError, or redirect attempt —
        // try next endpoint
        continue;
      }
    }
    if (attempt < MAX_RETRIES - 1) {
      await new Promise<void>((r) => setTimeout(r, RETRY_DELAY_MS));
    }
  }

  throw new AleaError(
    6100,
    "DrandFetchFailed: all drand endpoints failed after retries",
  );
}
