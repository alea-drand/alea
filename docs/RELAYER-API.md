# Alea Relayer — API contract (design spec)

Status: **design only**. Implementation queued for a separate session. This document pins the API shape now so the frontend design + copy can be built against a stable contract.

## Purpose

The splash page's `VERIFY A ROUND` interaction needs to trigger a **real** Solana verification of the latest drand round and show the resulting transaction. Three constraints shape the service:

1. **Visitor has no wallet.** Signing must happen server-side with a funded relayer keypair.
2. **No per-visitor cost on mainnet must be allowed.** Ops pays; rate-limit bounds worst case.
3. **Frontend must be honest.** Every value shown is real: real drand round, real Solana tx, real slot number.

## Deployment modes

| Phase | Network | Funding | Why |
|---|---|---|---|
| Phase 4 (now) | devnet | 1 SOL devnet (faucet-topped) | Alea only deployed to devnet; cost = 0 |
| Phase 5+ | mainnet-beta | 0.1 SOL float, monthly top-up from project wallet | Alea mainnet deploy ships; narrative upgrades from "demo" to "live" |

The mode switch is a single environment variable (`ALEA_CLUSTER`). Frontend code does not change between modes — all cluster-specific values flow through API responses.

## Endpoints

### `POST /api/verify`

Trigger a real on-chain verification of the current drand round.

**Request**
```json
POST /api/verify
Content-Type: application/json

{ }
```

(Body empty. We intentionally do not let the caller pick a round — the server always verifies the *latest* drand round. Prevents pre-computed replay spam.)

**Response — success (200)**
```json
{
  "ok": true,
  "round": 6039963,
  "rand": "0x7fdecf7e9d2a8b4c...e1f8db59967062",
  "sig":  "0x8ba4a6fd23e1c90a...7b5dfe5e9a",
  "tx":   "4tr4yetr4j3U9LjSNfA4CWVYNKrZr9nAKx51pV9baXJ7FAB1Hi4AjquAcQUcpgGD7buiGR9ppSbYTrVETnD9Zdtj",
  "slot": 312441952,
  "programId": "ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U",
  "cluster": "devnet",
  "computeUnitsUsed": 407128,
  "costLamports": 4520,
  "explorerUrl": "https://explorer.solana.com/tx/4tr4yetr...?cluster=devnet",
  "programUrl":  "https://explorer.solana.com/address/ALEAydzHd...?cluster=devnet",
  "verifiedAt":  "2026-04-19T14:22:47.182Z"
}
```

**Response — rate limited (429)**
```json
{
  "ok": false,
  "error": "rate_limited",
  "retryAfterSeconds": 4,
  "lastProof": {
    "round": 6039962,
    "rand": "...",
    "sig":  "...",
    "tx":   "...",
    "slot": 312441941,
    "verifiedAt": "2026-04-19T14:22:32.094Z"
  }
}
```

The `lastProof` fallback lets the frontend show *something* real rather than a dead-end error. Visitor rate-limited but still learns the system works.

**Response — relayer offline / funding exhausted (503)**
```json
{
  "ok": false,
  "error": "relayer_offline",
  "reason": "insufficient_funds" | "rpc_unavailable" | "maintenance",
  "lastProof": { ... same shape as above ... }
}
```

Frontend falls through to `lastProof` and shows a disabled button with the copy: `demo offline — see last proof ↗`.

### `GET /api/proofs/recent?limit=5`

Rolling verification log for the "last N verifications" strip shown below the main beacon module.

**Response (200)**
```json
{
  "ok": true,
  "cluster": "devnet",
  "proofs": [
    { "round": 6039963, "tx": "...", "slot": 312441952, "verifiedAt": "2026-04-19T14:22:47.182Z", "explorerUrl": "..." },
    { "round": 6039962, "tx": "...", "slot": 312441941, "verifiedAt": "2026-04-19T14:22:32.094Z", "explorerUrl": "..." },
    { "round": 6039961, "tx": "...", "slot": 312441930, "verifiedAt": "2026-04-19T14:22:17.008Z", "explorerUrl": "..." },
    { "round": 6039960, "tx": "...", "slot": 312441919, "verifiedAt": "2026-04-19T14:22:01.914Z", "explorerUrl": "..." },
    { "round": 6039959, "tx": "...", "slot": 312441908, "verifiedAt": "2026-04-19T14:21:46.822Z", "explorerUrl": "..." }
  ]
}
```

Backed by a ring buffer (last 100 proofs). Client-side requests `limit=5` for splash, bigger on `/history` page if we ever build one.

### `GET /api/health`

Offline detection + build info.

**Response (200 when healthy, 503 when not)**
```json
{
  "ok": true,
  "cluster": "devnet",
  "relayerBalance": 0.9834,
  "relayerBalanceFloor": 0.1,
  "lastVerifyAgeSeconds": 14,
  "buildSha": "8965062489fd",
  "uptimeSeconds": 412318
}
```

Frontend polls `/health` every 60s to decide whether VERIFY is enabled. When `ok:false`, button disables and falls through to last proof.

## Rate limiting

Per-IP token bucket, enforced server-side.

| Endpoint | Limit | Burst | Notes |
|---|---|---|---|
| `POST /api/verify` | 6/min | 3 | Plenty for legitimate curiosity; blocks basic bots |
| `GET /api/proofs/recent` | 60/min | 20 | Cache headers: `Cache-Control: public, max-age=5` |
| `GET /api/health` | 60/min | 20 | Poll target; cached |

Headers on every response:
```
X-RateLimit-Limit: 6
X-RateLimit-Remaining: 4
X-RateLimit-Reset: 1713547820
Retry-After: 8  (only on 429)
```

Frontend reads `Retry-After` to drive the button's disabled-countdown state.

**Future hardening** (Aaron's ask): per-IP rate limit is v1; add fingerprint-based throttling if IP-only is gamed, or Cloudflare Turnstile on the verify endpoint if abuse lands.

## Key management

- **Relayer keypair**: generated per environment, stored in the relayer process's env (`RELAYER_SECRET_KEY` as base58). Never committed. Never shared with frontend. On mainnet, store in 1Password + deploy via secrets mount — not in `.env` files on disk.
- **Cluster switch**: single env var `ALEA_CLUSTER=devnet|mainnet-beta`. RPC URL, program ID, and Explorer URL base all flow from this.
- **Program ID**: read from config not hardcoded, so the relayer survives a program upgrade.

## Devnet faucet topup (cron)

Hourly check, airdrop if balance < 0.5 SOL. Three-tier fallback per `framework-gotchas.md` and `2026-04-18-solana-devnet-faucet-globally-dry.md`:

```
1. Helius devnet faucet API (primary — authenticated, higher limits)
2. QuickNode faucet API (secondary — free tier, 1 SOL/day)
3. `solana airdrop 2 $RELAYER --url devnet` (tertiary — public, unreliable)
```

If all three fail 3× consecutively, alert to Slack/PagerDuty. Manual topup from Discord faucets (76Devs, LamportDAO) is the last resort and requires human.

## Transaction construction (implementation notes)

Not part of the API contract, but documented so the code phase inherits correct assumptions.

Per vault memory `solana-raw-instruction-error` + `anchor-030-web3js-198-incompat`, Anchor 0.30.1 + web3.js ≥ 1.98 cannot use `.rpc()`. Relayer must:

1. Build tx: `program.methods.verifyRound(round, signature).transaction()`
2. Sign: `relayerKeypair.signTransaction(tx)` or equivalent
3. Send: `connection.sendRawTransaction(serialized, { skipPreflight: true })`
4. Confirm: `getTransaction(sig, { commitment: "confirmed" })` with retry loop (Helius indexer lags 2-5s on confirmed commitment)
5. Extract: `meta.err` for success/failure, `meta.computeUnitsConsumed` for the `computeUnitsUsed` response field, `meta.fee` for `costLamports`

## Offline degradation

Every endpoint returns a usable fallback when it can:
- `verify` rate-limited → 429 with last proof embedded
- `verify` offline → 503 with last proof embedded
- `proofs/recent` stays available from the ring buffer even if relayer can't send new txs
- `health` says what's wrong, frontend reads + adapts

Frontend contract: **never show "error" UI**. Always show the most recent real proof as a fallback. Silent degrade is the design register for a museum-grade public good.

## What the frontend commits to

Independent of relayer state:

- Live drand beacon ticker (`api.drand.sh/public/latest`) always runs, never depends on our backend
- VERIFY button reflects the server's authoritative state (via `/health` + rate-limit headers)
- Post-click proof block links to Explorer using the real `explorerUrl` from the response
- The `X-RateLimit-Remaining` header drives subtle UX signals (no modal popups — just button disable state)

## Out of scope for this arc

- Relayer implementation (queued)
- Logging / analytics pipeline (separate concern)
- Admin UI for operator (probably just CLI scripts)
- Mainnet billing reconciliation (Phase 5+ concern)

## Related vault notes

- `project_drand_solana` — Alea protocol technical status
- `2026-04-18-solana-devnet-faucet-globally-dry` — faucet fallback playbook
- `2026-04-19-anchor-030-web3js-198-incompat` — why we bypass Anchor's `.rpc()`
- `2026-04-18-helius-devnet-indexer-lag` — confirmation retry loop rationale
- `2026-04-18-solana-raw-instruction-error-format` — error parsing for `skipPreflight:true` path
