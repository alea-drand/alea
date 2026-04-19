# @alea/sdk TypeScript Integration Audit

**Date:** 2026-04-19  
**Auditor:** Marco (senior TS/Node dev persona — first-time consumer, never seen codebase)  
**Method:** Full fresh-consumer install in `/tmp/alea-consumer-ts-audit/` using local package as npm install proxy.  
**Node version:** v25.8.1  
**Test environment:** macOS, Solana devnet  

---

## Scorecard

| Axis | Score | Notes |
|------|-------|-------|
| **Installability** | 9/10 | Zero npm warnings, zero peer dep errors. Clean 67-package install in 4s. |
| **30-second quick-start** | 7/10 | `fetchBeacon()` is instant. Full `getVerifiedRandomness` requires devnet SOL — faucet fails silently; README covers it but the path from zero is slow. |
| **API ergonomics** | 9/10 | Function names match intent. `bigint` for rounds is correct but unfamiliar. `Keypair | Wallet` union is exactly right. |
| **Error quality** | 6/10 | Validation errors (6102) are excellent. `verifyDrandBeacon` with 0-SOL keypair throws `6009 ReturnDataMissing` instead of "insufficient funds" — misleading. |
| **TypeScript experience** | 8/10 | No `any` in public surface. Strict clean with `skipLibCheck: true`. Without it, Anchor 0.32.1's own `@types/bn.js` gap produces 5 errors — not @alea/sdk's fault, but uncalled out in README. Function-level JSDoc missing on both main entry points. |
| **Browser compatibility** | 9/10 | No `node:*` imports anywhere in dist. IDL inlined as TS const (good). `dist/client.js` subpath inaccessible via exports map (minor). |
| **Completeness** | 8/10 | All exported symbols work. `isRoundRecent`, `createVerifyInstruction`, `getConfigAddress` all type-check. `MAINNET_PROGRAM_ID` Proxy throw-on-access is clever and correct. Missing: `tsconfig` guidance in README for strict mode consumers. |

**Overall: 8.0 / 10**

---

## Frictions Hit (Ordered by Severity)

### F1 — T2 — Misleading Error When Signer Has No SOL (error.ts, client.ts)

**What happened:** Called `verifyDrandBeacon` with a freshly generated keypair (0 SOL balance). The SDK sent the transaction (it was accepted by devnet RPC), then polled `getTransaction` 15 times over 15 seconds. When `getTransaction` returned `null` (tx dropped / failed due to insufficient fee), the SDK threw:

```
AleaError 6009: ReturnDataMissing: getTransaction returned null for sig 5EicNo2s...
```

**Why it matters:** `6009 ReturnDataMissing` reads like an on-chain problem. The real cause is "you don't have enough SOL to pay the fee." A first-time consumer debugging this at 1am will waste significant time.

**Root cause:** The 15-second polling loop exhausts without a result when the tx was never finalized, and the error falls through to the `info === null` branch (`ReturnDataMissing`).

**T2 Fix proposal:**  
In `client.ts`, before calling `sendRawTransaction`, preflight the fee payer's balance. If balance < estimated fee (~5000 lamports + cu_limit × price), throw:

```typescript
// in verifyDrandBeacon, after getting blockhash, before sendRawTransaction:
const balance = await args.connection.getBalance(wallet.publicKey);
if (balance < 5000) {
  throw new AleaError(6102, `InvalidInput: signer ${wallet.publicKey.toBase58()} has ${balance} lamports — need at least 5000 (+ compute budget) to pay tx fees`);
}
```

Or alternatively, differentiate `info === null` from `info.meta?.err !== null` in the polling exit:

```typescript
// After poll loop
if (!info) {
  throw new AleaError(
    6009,
    `Transaction not found after 15s — possible causes: insufficient SOL for tx fees, dropped by RPC, or network issue. Sig: ${tx}`
  );
}
```

**File:** `sdk/typescript/src/client.ts` — `verifyDrandBeacon`, line ~162 (`if (!info)`)

---

### F2 — T2 — No `skipLibCheck` Guidance in README (README.md)

**What happened:** Running `tsc --noEmit` with `skipLibCheck: false` (full strict mode) produces 5 errors — all from `@coral-xyz/anchor@0.32.1`'s own types:

- `TS7016: Could not find a declaration file for module 'bn.js'` (3 occurrences)
- `TS2344: Type 'AllEvents<IDL>[number]' does not satisfy constraint 'IdlEvent'` (2 occurrences)

These are **not @alea/sdk errors** — they are upstream Anchor bugs present in Anchor 0.32.1. However, a first-time consumer who runs `tsc` and sees 5 errors will assume the SDK is broken.

**Note:** With `skipLibCheck: true` (which is the default in CRA, Next.js, Vite, and most real-world tsconfigs), the SDK types are **completely clean** in strict mode. Zero errors. No `any` in the public surface.

**T2 Fix proposal:** Add a TypeScript section to the README Install block:

```markdown
### TypeScript

```json
// tsconfig.json — add this if you see Anchor-related type errors
{ "compilerOptions": { "skipLibCheck": true } }
```

`@coral-xyz/anchor@0.32` ships with a known `bn.js` type gap. `skipLibCheck: true` is safe and is the default in all major framework starter templates.
```

**File:** `sdk/typescript/README.md` — after the Install section

---

### F3 — T3 — Missing Function-Level JSDoc on `getVerifiedRandomness` and `verifyDrandBeacon`

**What happened:** In VS Code, hovering over `getVerifiedRandomness` shows only the type signature. There is no leading `/** */` doc block explaining what the function does, what it returns, or when to use it over `verifyDrandBeacon`.

Parameters have inline `/** ... */` comments (good), but `signal` and `skipPreflight` docs only appear on `verifyDrandBeacon`. On `getVerifiedRandomness` they are one-liners that reference the other function.

**T3 Fix proposal:** Add function-level JSDoc blocks to both signatures in `src/client.ts`:

```typescript
/**
 * High-level entry point. Fetches the latest drand beacon and submits a
 * Solana transaction that verifies it on-chain. Returns 32 bytes of
 * cryptographically verified randomness.
 *
 * @param options.connection - Solana RPC connection
 * @param options.signer - Keypair (server/Node) or wallet-adapter WalletContextState (browser)
 * @param options.programId - Override the default DEVNET_PROGRAM_ID
 * @param options.round - Specific drand round to verify (defaults to latest)
 * @param options.computeUnits - CU limit override (default 900_000)
 * @returns 32 bytes of on-chain verified randomness
 * @throws {AleaError} 6100 if drand fetch fails
 * @throws {AleaError} 6102 if inputs are invalid
 * @throws {AleaError} 6103 if AbortSignal fires before tx is sent
 */
```

**File:** `sdk/typescript/src/client.ts` — before both exported functions

---

### F4 — T3 — No Subpath Export for `dist/client.js` (browser-compat verification broken)

**What happened:**

```
node -e "import('@alea/sdk/dist/client.js').then(() => console.log('no fs'))"
// → Error: Package subpath './dist/client.js' is not defined by "exports"
```

The exports map only exposes `"."`. This is correct for consumers, but it means:
1. The "does it avoid fs?" browser-compat spot-check fails on a confusing error
2. Advanced consumers who want tree-shakeable subpath imports cannot access them

**T3 Fix proposal (optional):** Either add a `"./client"` subpath export or add a note in the README clarifying that `@alea/sdk` is a single-entry package (no subpath exports by design):

```json
"exports": {
  ".": { "types": "./dist/index.d.ts", "import": "./dist/index.js" },
  "./client": { "types": "./dist/client.d.ts", "import": "./dist/client.js" },
  "./drand": { "types": "./dist/drand.d.ts", "import": "./dist/drand.js" }
}
```

**Note:** `sideEffects: false` is already set, so bundlers will tree-shake unused exports from `"."` without needing subpath exports. This is T3 — quality-of-life only.

**File:** `sdk/typescript/package.json` — `exports` field

---

### F5 — T3 — Devnet Faucet Failure Has No Fallback Link in README Quick-Start

**What happened:** The "Testing on Devnet" section says:

```bash
solana airdrop 1 ~/.config/solana/alea-test.json --url devnet
# Backup faucet if solana-labs is dry: 76 Devs Discord / LamportDAO
```

The airdrop failed during this audit with `Internal error`. The README mentions backups but provides no links. A first-timer is stuck.

**T3 Fix:** Add direct links:

```markdown
# Backup faucets (solana-labs is frequently dry):
# - https://faucet.solana.com  (Solana web faucet, no login)
# - https://solfaucet.com      (1 SOL per request, no login)
# - 76Devs Discord: https://discord.gg/solana (ask in #devnet-faucet)
```

**File:** `sdk/typescript/README.md` — Testing on Devnet section

---

## What Worked Well

1. **Zero-friction install.** `npm install @alea/sdk @solana/web3.js @coral-xyz/anchor` resolved cleanly, no deprecated transitive deps, no peer dep conflicts. 67 packages in 4 seconds.

2. **ESM import is clean.** `import('@alea/sdk').then(m => Object.keys(m))` returns all 16 expected exports immediately. Dynamic import works in both ESM and CJS consumer packages.

3. **`fetchBeacon()` works instantly.** No wallet, no SOL, no setup. First-call latency ~300ms against the Cloudflare drand endpoint. Returns well-typed `DrandBeacon` with correct `bigint` round, `Uint8Array` signature, and labeled `unverifiedRandomness` string.

4. **Input validation errors are excellent.** All 4 negative cases tested threw `AleaError 6102` with precise, actionable messages:
   - `null` signer → `"signer is required (got null)"`
   - `number` round → `"round must be bigint (got number)"`
   - wrong sig length → `"must be exactly 64 bytes (G1 uncompressed x||y); got 32"`
   - `round=0n` → `"must be in [1, 2^64-1] (got 0)"`

5. **Zero `any` in public surface.** The entire `dist/*.d.ts` surface is fully typed. No implicit anys leaked from the Anchor IDL integration.

6. **No browser-hostile imports.** No `node:fs`, `node:path`, `node:crypto`, or `require()` calls in any dist file. The IDL is inlined as a TS const — the right call.

7. **`MAINNET_PROGRAM_ID` Proxy is clever and correct.** Throws on `.toBase58()`, `.toString()`, property access, everything — with a clear, actionable message. The `then` carve-out prevents accidental async hang. Exactly the right behavior.

8. **`isRoundRecent` semantics match Rust.** Tested: current round returns `true`, round=1 (very old) returns `false`. Future-round semantics (returns `true` to match Rust's saturating-sub) are documented inline.

9. **`ERRORS` map has actionable messages.** Every code has a description that tells you whether to retry or not (e.g., `"non-retryable"`, `"do not retry"`). This is excellent for UI/toast copy.

---

## Specific Recommendations Summary

| Priority | Fix | File | Estimated Effort |
|----------|-----|------|-----------------|
| T2 | F1: Improve `info === null` error to mention "insufficient SOL" | `src/client.ts:162` | 15 min |
| T2 | F2: Add `skipLibCheck: true` guidance to README | `README.md` | 5 min |
| T3 | F3: Add function-level JSDoc to `getVerifiedRandomness` + `verifyDrandBeacon` | `src/client.ts` | 30 min |
| T3 | F4: Add subpath exports OR document single-entry intent | `package.json` | 10 min |
| T3 | F5: Add direct faucet links to devnet testing section | `README.md` | 5 min |

---

## Publish-Ready Verdict

**YES — with caveats.**

The SDK is ready to publish to npm for the devnet phase. The core happy path works: `npm install` is clean, `fetchBeacon()` returns correct typed data in ~300ms, ESM import is clean, all input validation errors are precise and carry error codes, and the public type surface is strict-clean with `skipLibCheck: true`.

**Blocking for mainnet (Phase 5):** None of the frictions above are blockers for devnet publication. F1 (misleading `ReturnDataMissing` on 0-SOL) is the most damaging consumer-experience issue and should be fixed before mainnet, but it doesn't break the correct path.

**Must-fix before Phase 5 mainnet publish:**
- F1 (misleading error code)
- F3 (JSDoc on main entry points — expected quality bar for a production SDK)

**Nice-to-have before devnet publish:**
- F2 (skipLibCheck README note — will save consumer confusion)
- F5 (faucet links)
