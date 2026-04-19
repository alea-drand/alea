# Persona Audit: Kai Nakamura — Senior Frontend / Browser dApp Dev

**Persona:** 8 years React, 2 years Solana dApps. Stack: Vite + React 18 + TanStack Query + `@solana/wallet-adapter`. Mental model is bundle size, tree-shakability, SSR (Next.js App Router), wallet-adapter ergonomics, clean error surfaces for toast notifications, and cancellable async flows.

**Scope read:** `sdk/typescript/src/`, `sdk/typescript/dist/`, `sdk/typescript/package.json`, `sdk/typescript/README.md`, `sdk/typescript/CAVEATS.md`, root `README.md`.

**Execution notes:** Read all source files and the compiled dist. No code changed. Probed all 10 questions in the brief. Findings are tiered T1 (SSR/bundler/runtime blocker) → T2 (real browser DX issue) → T3 (polish).

---

## Findings

**[T1] `sdk/typescript/src/client.ts:9-11` — `readFileSync`, `fileURLToPath`, `path` are top-level imports that execute at module load time**

`import { readFileSync } from "fs"` and `import { dirname, join } from "path"` are unconditional CJS/Node builtins at the top of `client.ts`. In a Next.js App Router environment, any component or server action that imports `@alea/sdk` (or anything that re-exports from `index.ts`) will cause the bundler to pull in `fs` and `path` at parse time. Vite/webpack with `browser` target will either fail with "Module not found: Can't resolve 'fs'" or silently inject an empty shim that breaks at runtime when `readFileSync` is called. The IDL is only 9KB — inline it as a plain JS object literal (`const IDL = { ... } as const`) during the build step and remove all three Node builtins from `client.ts` entirely. This is the single highest-priority fix for any browser-side dApp integration.

**[T1] `sdk/typescript/dist/client.js:3-5` — compiled output confirms the SSR blocker ships in the tarball**

The compiled `dist/client.js` retains `import { readFileSync } from "fs"`, `import { fileURLToPath } from "url"`, and `import { dirname, join } from "path"` verbatim. The comment in source explains the intent (avoid Node 18/22 JSON import syntax split) but the chosen fix trades one Node-version problem for a hard bundler break in every browser build tool. The correct fix (inline IDL object) is both simpler and universally portable.

**[T2] `sdk/typescript/src/drand.ts:66-97` — `fetchBeacon` retry loop has no `AbortSignal` threading; navigation away mid-fetch does not cancel the pending Solana tx**

Each individual `fetch` call uses `AbortSignal.timeout(TIMEOUT_MS)` for its own 5s window, which is correct. But `fetchBeacon` itself accepts no external `AbortSignal`, and neither does `verifyDrandBeacon` or `getVerifiedRandomness`. A user who navigates away during the 77-second worst-case drand fetch loop gets no cancellation — and once the drand fetch succeeds, the Solana tx is dispatched unconditionally. In a prediction market UI, that means a stale position commit can land on-chain after the user has left the page. Both `fetchBeacon` and `getVerifiedRandomness` should accept an optional `AbortSignal` that is threaded into each `fetch` call and checked before dispatching `sendRawTransaction`.

**[T2] `sdk/typescript/src/client.ts:129` — `wallet.signTransaction` is called without checking capability; missing wallets throw opaque errors**

The `isBrowserWallet` discriminant (line 28) checks for `"sendTransaction" in signer`, which matches `WalletContextState` from `@solana/wallet-adapter-base`. However, some wallet-adapter implementations (hardware wallets, read-only watch wallets) may not implement `signTransaction` even though they satisfy the `Wallet` interface. The call at line 129 (`const signedTx = await wallet.signTransaction(anchorTx)`) will throw a wallet-specific error ("Method not supported", "Not implemented", etc.) that does not surface as an `AleaError` with a useful code. A `typeof wallet.signTransaction !== "function"` guard with a thrown `AleaError` (or at minimum a descriptive `Error`) before the call would give the toast a useful message.

**[T2] `sdk/typescript/src/client.ts:104` — 900k compute-unit default is silent to the caller**

`computeUnits` defaults to `900_000` silently. The README documents this in the options table but there is no runtime feedback: the SDK adds a `setComputeUnitLimit` instruction unconditionally and the caller has no way to know it happened without reading source. For a browser dApp doing fee estimation (e.g., using `getPriorityFeeEstimate` before signing), the invisible 900k floor means fee estimates are valid but the actual tx always requests the full ceiling. Consider logging a `console.debug` in development, or exposing a `getDefaultComputeUnits()` constant so callers can reason about it without reading source.

**[T2] `sdk/typescript/src/client.ts:134` — `skipPreflight: true` is unconditional with no debug escape hatch**

`sendRawTransaction` always passes `{ skipPreflight: true, maxRetries: 3 }`. The comment explains why (pairing CU outpaces preflight blockhash window), which is a legitimate constraint. But there is no way for an integrator to opt into preflight for debugging without forking the SDK. A `{ debug?: boolean }` option that flips to `skipPreflight: false` — even with a documented warning that it may spuriously fail — would meaningfully improve the debugging story for dApp developers hitting unexpected failures.

**[T2] `sdk/typescript/src/client.ts:155-161` — 15-second polling loop has no AbortSignal and no progress callback**

After tx send, the code polls `getTransaction` up to 15 times with 1-second sleeps. There is no way to cancel this loop from outside, no progress event emitted, and no way for a TanStack Query `queryFn` to respond to query cancellation during this window. In practice this means a React component that unmounts or a query that is cancelled will leave a dangling Promise polling the RPC for up to 15 more seconds.

**[T2] `sdk/typescript/src/client.ts:199-200` — `Buffer.from(dataB64, "base64")` relies on Node's `Buffer` global**

The return-data decode path uses `Buffer.from(dataB64, "base64")` (line 199) and `Buffer.from(dataB64, "base64")` again at line 157 in dist. In a browser environment where `Buffer` is not natively available, this requires a polyfill (e.g., Vite's `buffer` plugin or webpack's `resolve.fallback`). The fix is `Uint8Array.from(atob(dataB64), c => c.charCodeAt(0))` which is natively available in every modern browser and in Node 16+. The `instruction.ts` file also uses `Buffer.from` and `Buffer.alloc` throughout (lines 31-37), compounding the surface area.

**[T3] `sdk/typescript/src/index.ts` — no subpath export for `fetchBeacon`-only use case; tree-shake requires bundler ESM support**

The `package.json` exports map only exports `.` (full barrel) and `./idl/alea_verifier.json`. A caller importing only `fetchBeacon` from `@alea/sdk` will pull in the full barrel including `client.ts`, which has the `fs`/`path` imports. Even with a bundler that supports ESM tree-shaking, the top-level side-effect (`readFileSync(idlPath)` executed at module load) defeats dead-code elimination for the entire `client.ts` module. A separate subpath export (e.g., `@alea/sdk/drand`) exposing only `drand.ts` and `errors.ts` would let fetch-only consumers avoid the Node builtins entirely.

**[T3] `sdk/typescript/src/client.ts:134` — `sendRawTransaction` returns a base58 signature string; Explorer link generation works as-is**

The `tx` variable holds the base58 transaction signature returned by `connection.sendRawTransaction`. Solana Explorer links are `https://explorer.solana.com/tx/{tx}?cluster=devnet` — the string can be appended directly without transformation. This is fine. Documented here only to close probe item 10 explicitly.

**[T3] `sdk/typescript/README.md:58` — `commitment` option listed in API reference but not accepted by `getVerifiedRandomness`**

The README documents a `commitment?: Commitment` option for `getVerifiedRandomness` (line 58), but the actual function signature in `client.ts` (line 211-217) does not accept or use a `commitment` parameter. The hardcoded `"confirmed"` is passed to `AnchorProvider` regardless. This is a docs/code mismatch — either add the parameter or remove it from the README.

---

## Summary

Two T1 blockers: `fs`, `path`, and `url` are unconditional top-level imports in `client.ts`, breaking every browser bundler and Next.js App Router at parse time. The compiled `dist/` ships this unchanged. Both stem from the IDL-loading strategy — inlining the IDL as a JS object literal fixes both simultaneously. Five T2 issues follow: no `AbortSignal` threading (stale tx risk), missing `signTransaction` capability guard, silent 900k CU default, no debug preflight escape hatch, and `Buffer` polyfill dependency in the return-data decode path. One T3 docs mismatch on the `commitment` option.
