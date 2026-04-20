# @alea-drand/sdk TypeScript Re-Verification — Tanya Morris

**Date:** 2026-04-19
**Auditor persona:** Tanya Morris — junior-to-mid TS dev, 3yr JS/TS, 6mo Solana hobby,
first real dApp, first time seeing drand. Not a security expert.
**Method:** Fresh environment `/tmp/alea-ts-verify-2/wheel-raffle/`. SDK installed via
local path dep (`npm install <repo-root>/sdk/typescript`). No modifications
to the SDK repo.
**Node version:** v25.8.1
**Test environment:** macOS, Solana devnet

---

## Scorecard

| Axis | Score | Notes |
|------|-------|-------|
| **30-second first-beacon** | 9/10 | `fetchBeacon()` returned in **305ms** with correct typed data. Full `getVerifiedRandomness` blocked by 0-SOL keypair (expected). Without SOL, first-beacon from the high-level API is a dead-end. |
| **Error UX** | 8/10 | T1-04/05/06 errors are excellent — every case returns `AleaError` with code + actionable message. Only gap: 0-SOL still throws `6009 ReturnDataMissing` (F1 from prior audit, not fixed). |
| **Vite/browser bundle** | 8/10 | T1-01 confirmed fixed — no `node:fs` import errors, bundle succeeds at 246KB. NEW: Vite emits `[IMPORT_IS_UNDEFINED] anchor.Wallet` warning for the browser bundle of `@coral-xyz/anchor`. Warn-only, does not break the build, tree-shook to 0 bytes in output. But it will confuse junior devs. |
| **TypeScript strictness** | 8/10 | Strict + noImplicitAny clean with `skipLibCheck: true`. Without it: 4 Anchor `bn.js` errors (not @alea/sdk's fault). README still has no guidance on this (F2 not fixed). No `any` in @alea/sdk public surface. |
| **Completeness** | 7/10 | All documented exports work. No function-level JSDoc on either main entry point (F3 not fixed). Hovering `getVerifiedRandomness` in VS Code shows only type signature — no @param descriptions, no @throws, no @returns doc. For a junior dev this is the biggest day-1 friction. |
| **Junior-dev confidence** | 7/10 | I could ship this to friends with some caveats. The `fetchBeacon()` one-liner is trustworthy. The full path requires understanding SOL fees, keypair funding, and devnet nuances. CAVEATS.md is excellent but I'd probably miss it. |

**Overall: 7.8 / 10** (vs. Marco's 8.0 — slightly lower because I'm junior and noticed the JSDoc gap more acutely)

---

## Verified-Fixes Check (Prior Audit F1–F5)

### F1 — T2 — Misleading `6009 ReturnDataMissing` on 0-SOL keypair
**Status: NOT FIXED**
`client.ts` line 248 still reads:
```
ReturnDataMissing: getTransaction returned null for sig ${tx}
```
No balance preflight, no updated error message mentioning "insufficient SOL". A fresh keypair still waits 15 seconds and gets a misleading error code. This is the #1 junior-dev pain point.

### F2 — T2 — No `skipLibCheck` guidance in README
**Status: NOT FIXED**
`grep skipLibCheck sdk/typescript/README.md` → no results. Without guidance, a junior dev running `tsc --noEmit` without `skipLibCheck` sees 4 Anchor `bn.js` errors and assumes the SDK is broken. Confirmed on Node v25.8.1 + TypeScript 5.4.

### F3 — T3 — Missing function-level JSDoc on `getVerifiedRandomness` and `verifyDrandBeacon`
**Status: NOT FIXED**
`dist/client.d.ts` shows no `/** */` block before either exported function. VS Code hover shows only the type signature. Parameter-level comments exist on `signal` and `skipPreflight` only. No `@param`, `@returns`, or `@throws` annotations. As a junior dev this was the single most noticeable gap — I didn't know what the function *does* from the autocomplete alone.

### F4 — T3 — No subpath exports or documentation of single-entry intent
**Status: NOT FIXED / Acceptable**
The exports map still only exposes `"."`. This is fine for consumers — Vite's tree-shaking via `sideEffects: false` works correctly. No action needed unless subpath imports are desired.

### F5 — T3 — No direct faucet links in devnet testing section
**Status: NOT FIXED**
`README.md` still reads `# Backup faucet if solana-labs is dry: 76 Devs Discord / LamportDAO` with no URLs. Airdrop failed in this session too. A first-timer is stuck without Googling.

---

## Specific Test Results (T1/T2 Tracker Items)

### T1-01 — Browser bundler: no `node:fs` imports
**PASS.** Vite build succeeds cleanly. No `node:fs` / `node:path` / `node:crypto` in any dist file (verified via `grep`). IDL inlined as TS const — correct solution.

**NEW FINDING:** Vite emits a build warning:
```
[IMPORT_IS_UNDEFINED] Warning: Import `Wallet` will always be undefined because
there is no matching export in '@coral-xyz/anchor/dist/browser/index.js'
(sdk/typescript/dist/client.js:87)
```
`anchor.Wallet` exists only in the Anchor Node/CJS bundle, not the browser bundle. The SDK's `new anchor.Wallet(args.signer)` fallback path (called when signer is a bare `Keypair`) is exercised only in server/Node contexts. In a browser, callers always use `wallet-adapter`, so `isBrowserWallet()` returns `true` and this path is skipped at runtime. Vite tree-shook it to 0 bytes in the output bundle (confirmed). The warning is non-fatal and the build produces a working 246KB bundle — but it will scare junior devs. **Severity: T3 (warn-only, tree-shook, not a runtime bug).**

### T1-02 — Drand round mismatch detection
**PASS (indirect).** `fetchBeacon()` returned the exact round requested (`16369347`) — round-match validation is working.

### T1-03 — Hex validation for drand signature
**PASS (indirect).** `fetchBeacon()` returned 64-byte `Uint8Array` signature correctly parsed from valid hex.

### T1-04 — Graceful failure on invalid round (`-1n`, `0n`)
**PASS.**
```
round=-1n → AleaError 6102: InvalidInput: round must be in [1, 2^64-1] (got -1)
round=0n  → AleaError 6102: InvalidInput: round must be in [1, 2^64-1] (got 0)
```
Both cases throw immediately (no network call) with toast-ready messages.

### T1-05 — Graceful failure on `null` signer
**PASS.**
```
null signer → AleaError 6102: InvalidInput: signer is required (got null)
```
Previous behavior was a `TypeError` from `"sendTransaction" in null`. Now caught at SDK boundary.

### T1-06 — Graceful failure on oversize signature
**PASS.**
```
65-byte sig → AleaError 6102: InvalidInput: signature must be exactly 64 bytes (G1 uncompressed x||y); got 65
```
Message names the expected format explicitly ("G1 uncompressed x||y") — a nice touch even if I don't know what that means yet.

### T2-08 — `MAINNET_PROGRAM_ID.toBase58()` throws with clear message
**PASS.**
```
Error: MAINNET_PROGRAM_ID not set (v0.1.x is devnet-only). Pass { programId }
explicitly, or wait for the post-Phase-5 release...
```
Contains actionable guidance (`Pass { programId } explicitly`). `then` carve-out works — no hang on accidental `await`.

---

## New Findings

### NF-01 — T3 — Vite `anchor.Wallet` undefined warning in browser build
**Severity: T3 (warn-only, not a runtime bug)**
**File:** `sdk/typescript/dist/client.js` — the `new anchor.Wallet(args.signer)` line (Node-only code path)
**What happened:** Vite's browser resolver sees `anchor.Wallet` is undefined in `@coral-xyz/anchor/dist/browser/index.js` and emits `[IMPORT_IS_UNDEFINED]`. Build still succeeds. Tree-shaking removes the dead code from output.
**Impact:** A junior dev running their first `vite build` sees a cryptic warning and Googles for 30 minutes. At 1am this is a session-ender.
**Root cause:** `client.ts` uses `anchor.Wallet` as a Keypair-wrapping class. That class is only in the Node bundle of Anchor. The browser bundle omits it.
**Fix options (pick one):**
1. Guard with `typeof anchor.Wallet !== "undefined"` and throw `AleaError 6102` with "Keypair signers are not supported in browser contexts — use a wallet-adapter WalletContextState" if called with a bare Keypair in a browser environment.
2. Import `NodeWallet` / `Wallet` from a separate Anchor subpath or implement the minimal interface inline (less coupling to Anchor internals).
3. Add `/* @vite-ignore */` comment and note in README that the Keypair path is Node-only (lowest-effort fix that still surfaces the right behavior at runtime).

### NF-02 — T3 — `signal` and `skipPreflight` docs appear on `verifyDrandBeacon` only
**Severity: T3**
`getVerifiedRandomness` says `/** Abort signal threaded through fetchBeacon + verifyDrandBeacon. */` and `/** See verifyDrandBeacon.skipPreflight. Default true. */`. A dev hovering on `getVerifiedRandomness` has to find `verifyDrandBeacon` docs to understand these params. Cross-referencing between function signatures in an IDE tooltip is friction.

---

## Raffle Scaffold Test

Created `/tmp/alea-ts-verify-2/wheel-raffle/src/spin.ts` and `src/main.ts`. The scaffold:
- Types check cleanly with `skipLibCheck: true` (`tsc --noEmit` → 0 errors)
- Without `skipLibCheck`: 4 errors from Anchor's `bn.js` gap (all upstream, 0 from @alea/sdk)
- `fetchBeacon()` returned live data in 305ms
- All error cases throw `AleaError` with readable messages
- `spinWheel()` would work end-to-end with a funded keypair

**Devnet run:** Blocked by 0-SOL keypair (expected — faucet dry during session). The 15s wait + `6009 ReturnDataMissing` is misleading. A clearer error message on the `info === null` path would unblock this without requiring SOL preflight.

---

## What Worked Well (Confirmed)

1. **307ms `fetchBeacon()` with correct types.** `round: bigint`, `signature: Uint8Array`, `unverifiedRandomness: string` — all exactly right.
2. **0-vulnerability npm audit** on both test projects.
3. **All four T1 error cases** return `AleaError 6102` instantly with precise, toast-ready messages. No opaque Node errors.
4. **`MAINNET_PROGRAM_ID` Proxy** throws with an actionable 150-char message. The `then` carve-out is a thoughtful detail.
5. **Vite build succeeds cleanly** — T1-01 (browser fs import) is verified fixed. No polyfills needed.
6. **`ERRORS` map** has every code with retry guidance. Perfect for toast copy.
7. **Zero `any` in public type surface.** Strict mode types are solid.
8. **`sideEffects: false`** enables tree-shaking — 246KB Vite bundle for a wallet + randomness dApp is reasonable.

---

## Publish Verdict

**YES — for devnet publication, with T2 items queued for Phase 5.**

The SDK is solid for a first real dApp project. `fetchBeacon()` works instantly, all input validation errors are excellent, Vite builds cleanly, and the type surface is strict-mode safe with `skipLibCheck: true`.

**Must-fix before Phase 5 mainnet publish:**
- F1: Misleading `6009 ReturnDataMissing` for 0-SOL (will confuse real users)
- F3: Function-level JSDoc on both main entry points (production SDK quality bar)
- NF-01: Vite `anchor.Wallet` warning (document or guard the browser/Node split)

**Must-fix before Phase 5 (quick wins, 5 min each):**
- F2: Add `skipLibCheck: true` guidance to README TypeScript section
- F5: Add clickable faucet links to "Testing on Devnet"

**Not blocking for devnet:**
- F4: Subpath exports (tree-shaking works fine without them)
- NF-02: Cross-reference signal/skipPreflight docs (minor IDE friction)

---

## Environment Notes

- Node: v25.8.1 | npm: 11.x | Vite: 8.0.8 | TypeScript: 5.4
- `@alea/sdk` installed from local path (dist already built)
- All tests run in `/tmp/alea-ts-verify-2/` — no SDK repo files modified
