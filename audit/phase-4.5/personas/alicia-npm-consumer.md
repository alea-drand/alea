# Persona: Alicia Park — Senior TypeScript / Solana Dev, npm consumer

**Background:** 12 years JS/TS, 4 years Solana. Works at a wallet startup. Evaluating `@alea/sdk` as a drand randomness source for an in-app loot-box feature. Node 20, devnet keypair, 30 minutes.

**Method:** Read README, simulate quick-start on paper, trace types through IDE autocomplete mental model, probe error paths and bundle shape against past SDK pain points. Compared against Switchboard and ORAO VRF DX baselines.

**Files read (npm-install simulation):** `package.json`, `README.md`, `CAVEATS.md`, `dist/index.d.ts`, `dist/client.d.ts`, `dist/drand.d.ts`, `dist/constants.d.ts`, `dist/errors.d.ts`, `dist/instruction.d.ts`, `src/client.ts`, `src/drand.ts`, `src/instruction.ts`, `src/constants.ts`, `src/errors.ts`, `src/index.ts`, `src/types.ts`, root `README.md`, `CHANGELOG.md`.

---

## Execution Notes

### 1. Install + PeerDeps

`package.json` declares `@solana/web3.js ^1.95`, `@coral-xyz/anchor ^0.30.1`, `@noble/hashes ^1.5` as hard `dependencies` — not peer deps. For a wallet startup already using web3.js ^1.98, `@coral-xyz/anchor` will land as a second Anchor copy if the app already pins a different version. The `peerDependenciesMeta` block names `@solana/wallet-adapter-base` as optional but lists no actual `peerDependencies` key, so npm will never emit a peer dep warning about it. There is no peer dep on `@solana/web3.js` itself, meaning version conflicts between the app's web3.js and the SDK's bundled copy are silent.

### 2. 30-Second Quick-Start Simulation

README opens with a CAVEATS link (good) then two quick-start blocks: browser and server. Both compile on paper. The browser example calls `useWallet()` at module scope outside a component — that's a React hook rules violation, but it's an illustrative snippet, not production guidance. The root `README.md` quick-start has a comment `// programId defaults to mainnet` which is misleading: it actually defaults to `DEVNET_PROGRAM_ID`, not mainnet. This is a subtle but material misread risk — a dev could ship pointing at devnet in production without realizing it.

### 3. Error Handling

`AleaError` is a proper class with `readonly code: number`. The error table in `ERRORS` is frozen at module load (good). Network failure from `fetchBeacon` throws `AleaError(0, "All drand endpoints failed")` — code `0` is not in the `ERRORS` map, so a consumer doing `ERRORS[err.code]` gets `undefined`. On-chain failures surface cleanly via `getTransaction().meta.err` extraction with fallback to log-scan — this is solid work and handles the web3.js 1.98 compat break correctly. The one gap: when `confirmTransaction` rejects and `getTransaction` returns `null` after 15 retries, the SDK throws `AleaError(6009, "ReturnDataMissing: getTransaction returned null for sig <sig>")`. The signature is in the message but not as a named field on the error object. A consumer writing a retry handler has to string-parse the signature back out.

### 4. Type Ergonomics

`signer: Keypair | Wallet` — `Wallet` here is `@coral-xyz/anchor`'s `Wallet` type, not `WalletContextState` from `@solana/wallet-adapter-react`. The d.ts declares `import type { Wallet } from "@coral-xyz/anchor"` which resolves correctly, but a developer who imports `useWallet()` from wallet-adapter-react gets a `WalletContextState` which satisfies `Wallet` structurally (both have `sendTransaction`) but only at runtime — the IDE will show a type error at the call site because `WalletContextState.sendTransaction` has a different signature than `AnchorWallet.sendTransaction`. The discriminant (`"sendTransaction" in signer`) works at runtime but the mismatch will cause TS compile errors for most wallet-adapter users without a cast.

`MAINNET_PROGRAM_ID` types as `PublicKey` in d.ts (`dist/constants.d.ts:7`) but throws at runtime on any property access. The Proxy trick is clever but the declared type is a lie — TypeScript will not warn on `MAINNET_PROGRAM_ID.toBase58()` even though it throws. A `never`-typed or `unique symbol` approach would surface this at compile time instead of runtime.

### 5. API Hierarchy Clarity

`getVerifiedRandomness` → `verifyDrandBeacon` → `createVerifyInstruction` is a clean three-tier ladder. The README documents all three with appropriate "use X when Y" framing. One ambiguity: `verifyDrandBeacon` is described as "canonical IDL-based instruction builder" but it actually builds AND sends the transaction and returns randomness — the word "builder" undersells it and creates a false symmetry with `createVerifyInstruction` (which actually is just a builder). A developer reading top-to-bottom might skip `verifyDrandBeacon` and reach for `createVerifyInstruction` unnecessarily.

### 6. Bundle Size / Files Field

`"files": ["dist/**", "src/idl/**", "README.md", "CAVEATS.md", "LICENSE"]` — `dist/**` includes all `.js.map` and `.d.ts.map` files. Source maps ship in the npm tarball (adds ~50% to download weight). For a crypto SDK this is fine; just notable. `src/idl/**` is also shipped alongside `dist/idl/**` — the IDL JSON is included twice. Not breaking, but adds ~4KB of duplication. `node_modules/` is correctly excluded. `tsconfig.json` and test files do not ship. Bundle is otherwise trim.

### 7. Node Version Compat

`"engines": { "node": ">=18" }` is correct. The `readFileSync`-based IDL load (`client.ts:21-24`) explicitly sidesteps the `import assert / with` JSON syntax fragmentation across Node 18-22+. `AbortSignal.timeout()` is Node 17.3+, so the `>=18` floor covers it. ESM-only (`"type": "module"`) is declared. No CJS export in the `exports` map — this will break any Webpack 4 / Jest (without experimental-vm-modules) / require() consumer. No warning in README about ESM-only. Webpack 5 (mode: esm) and Vite are fine; CRA with Webpack 4 will hard-fail at import resolution.

### 8. MAINNET_PROGRAM_ID Proxy Behavior

Imports fine. Any property access throws a descriptive error. The `Symbol.toPrimitive`, `toString`, and `then` guard prevents the object from being accidentally awaited or serialized in unexpected ways (good). The only gap: `MAINNET_PROGRAM_ID.constructor` would throw too (not guarded), which could confuse `instanceof` checks in downstream tooling. Minor, not user-facing.

### 9. Dev-Only Code in Source

No `console.log` in any `src/` file. The devnet test file uses `console.warn` for skip-logging which is gated behind `ALEA_DEVNET_TESTS=1` and lives in `tests/` (not shipped). Clean.

### 10. Commitment Configurability

`verifyDrandBeacon` hardcodes `commitment: "confirmed"` inside `AnchorProvider` construction (`client.ts:88`) and inside `getTransaction` (`client.ts:157`). The `getVerifiedRandomness` options type exposes a `commitment?: Commitment` field in the README API block but the actual function signature in `src/client.ts:211-218` does NOT include `commitment` — it accepts `connection, signer, programId, round, computeUnits`. The README documents a parameter that doesn't exist in the implementation.

---

## Tiered Findings

**[T1] `sdk/typescript/README.md` (root `README.md`:87) — `// programId defaults to mainnet` comment is wrong; actual default is `DEVNET_PROGRAM_ID`.**
A developer copying the root README quick-start will read that omitting `programId` uses mainnet. It actually uses devnet. This is a shipping risk: a loot-box on mainnet silently pointing at a devnet program would fail every transaction with `AccountNotFound` and the dev would waste time chasing the wrong root cause.

**[T1] `sdk/typescript/README.md:53-58` + `src/client.ts:211` — `commitment` option documented but not implemented.**
The API reference block shows `commitment?: Commitment` as an accepted option for `getVerifiedRandomness`. The actual function signature does not accept or use this parameter. Commitment is hardcoded to `"confirmed"` inside `verifyDrandBeacon`. A consumer following the docs and passing `{ commitment: "finalized" }` gets silent non-effect — no TS error (extra properties in object literals are erased), no runtime warning.

**[T2] `sdk/typescript/dist/constants.d.ts:7` — `MAINNET_PROGRAM_ID: PublicKey` type declaration does not reflect runtime throwing behavior.**
The declared type says `PublicKey`, so TypeScript autocompletes `.toBase58()`, `.equals()`, etc. without complaint. The Proxy throws on any property access at runtime. A `@ts-expect-error` + opaque type or `never` would surface this mistake at compile time. As-is, the Proxy provides runtime protection but zero compile-time protection.

**[T2] `sdk/typescript/src/errors.ts:1` + `sdk/typescript/src/drand.ts:100` — network-failure error uses code `0` which is absent from `ERRORS` map.**
`fetchBeacon` throws `new AleaError(0, "All drand endpoints failed after retries")`. Code `0` has no entry in `ERRORS`. A consumer writing `catch (e) { if (e instanceof AleaError) logError(ERRORS[e.code]) }` gets `undefined` for network failures. Either add a code (e.g., `5000: "NetworkError"`) or document that `0` is the sentinel for network failures.

**[T2] `sdk/typescript/src/client.ts:199` + error shape — transaction signature not a named field on `AleaError`.**
When `getTransaction` returns null after exhausting retries, the tx signature appears in `err.message` but not as a dedicated property. A consumer writing retry or alerting logic has to regex the message string to recover the signature. Add `readonly txSignature?: string` to `AleaError`.

**[T2] `sdk/typescript/src/client.ts:28` + `dist/client.d.ts:1` — `Wallet` type is Anchor's type, not wallet-adapter's `WalletContextState`.**
The d.ts imports `Wallet` from `@coral-xyz/anchor`. Wallet-adapter's `useWallet()` returns `WalletContextState` which has a structurally incompatible `sendTransaction` signature. Most Solana wallet-adapter apps will get a TS compile error when passing the wallet hook result to `signer`. The browser quick-start example in the SDK README would fail to compile for a typical Next.js wallet-adapter user without a cast. Consider documenting the required adapter shim or accepting `Pick<WalletContextState, 'publicKey' | 'signTransaction'>`.

**[T2] `sdk/typescript/package.json` — no `peerDependencies` key; `@solana/web3.js` and `@coral-xyz/anchor` ship as hard deps.**
For an SDK, bundling `@solana/web3.js` and Anchor as dependencies (not peers) means every consumer project carries a second copy of both. Wallet apps that already have web3.js will get two copies in the bundle — type conflicts, larger bundle, potential pubkey/transaction class identity issues (`instanceof PublicKey` fails across module copies). Standard SDK practice is to peer-dep on `@solana/web3.js` and Anchor.

**[T3] `sdk/typescript/README.md:64` — `verifyDrandBeacon` described as "instruction builder" but it sends the transaction.**
The word "builder" implies it returns an instruction or unsigned tx. It actually fetches the latest blockhash, signs, sends, polls, and returns 32 bytes. The confusion between `verifyDrandBeacon` (full send) and `createVerifyInstruction` (actual builder) will cause devs to reach for the wrong one. Rename the description to "Fetch, sign, send, and confirm" or similar.

**[T3] `sdk/typescript/package.json:32-38` — IDL JSON shipped twice (`src/idl/**` and inside `dist/**`).**
Both `dist/idl/alea_verifier.json` and `src/idl/alea_verifier.json` land in the tarball. The `src/idl/**` entry in `files` is unnecessary since `dist/idl/` is already included and the SDK's public API loads only from `dist/`. Remove `src/idl/**` from `files`.

**[T3] `sdk/typescript/package.json` — no note about ESM-only in README.**
`"type": "module"` + no CJS export map entry means `require('@alea/sdk')` throws. No warning in README. CJS users (Webpack 4, older Jest configs, non-bundler Node scripts with `require()`) will hit `ERR_REQUIRE_ESM` with no explanation. One sentence in the install section would save a support ticket.

---

## Summary

The SDK's happy path is solid: quick-start compiles on paper, error extraction from on-chain failures is carefully engineered (the web3.js 1.98 workaround is the right call), and there is no dev-only noise in shipped code. Two issues need fixing before npm publish: the root README comment claiming mainnet default (T1 — will cause silent wrong-network failures in production) and the `commitment` option documented but not wired (T1 — silent non-effect misleads callers). The Anchor/wallet-adapter type mismatch (T2) will block most browser wallet-adapter integrations at compile time. Hard-dep on web3.js and Anchor instead of peer deps is the biggest structural issue for a published SDK — it should be reclassified before a 1.0.
