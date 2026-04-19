# Phase 4.5 — Consolidated Audit Findings

**Date:** 2026-04-19
**Branch:** feature/phase-4.5-audit
**Audits run:** 8 personas + 4 red-team agents = 12 independent cold reads
**Total unique findings:** 16 T1, ~28 T2, ~25 T3 (deduped across agents)
**Critical T1s (crypto, replay, client security):** 0 found
**Publish-blocking T1s (will cause real consumer breakage or publish failure):** 16

All agents worked materials-only (sdk/ + programs/ + README, no build-spec access) to simulate external-consumer view.

## Executive summary

**Security posture is strong.** The 3 core crypto/security guardrails hold:
- `seeds::program` mandatory constraint (ADR 0034) enforces PDA ownership
- `is_round_recent` saturating arithmetic is overflow-safe
- Return-data ordering discipline is correctly documented and demonstrated
- Crypto boundaries (G1/G2, pairing, SVDW, sha256 per ADR 0036) pass edge-case review

**The T1s are NOT about crypto — they're about publish quality.** Documentation drift, input validation gaps, publish-metadata bugs, browser-bundler blockers, and one real mempool concern (drand endpoint round substitution).

**12 agents, 0 disagreements on any T1.** When multiple personas surface the same finding they converge on the same fix — a sign the issues are real and unambiguous.

## T1 findings (publish blockers)

### T1-01 — Browser SSR/bundler blocker: Node built-ins at module top-level
**File:** `sdk/typescript/src/client.ts:9-11`
**Surfaced by:** Kai, Nia (both T1), Raul (indirectly)
**Issue:** `readFileSync`, `fileURLToPath`, `dirname`, `join` imported at top level. Any browser bundler (Vite, esbuild, webpack App Router) fails at build time when `@alea/sdk` is imported. README shows browser quick-start, so this is a direct contradiction.
**Fix options (TASTE DECISION — surface to Aaron):**
- (a) Inline IDL at build time — remove all three Node imports (Kai's rec)
- (b) `package.json` `browser` field pointing to a separate entry
- (c) Conditional dynamic import() that only loads fs in Node context
- (d) Factory function that accepts pre-parsed IDL
**Recommendation:** (a) inline IDL. Simplest, zero runtime overhead, zero Node deps. IDL updates need rebuild — acceptable trade-off.

### T1-02 — `fetchBeacon` doesn't verify returned round matches requested round
**File:** `sdk/typescript/src/drand.ts:79-83`
**Surfaced by:** Fen (red-team)
**Issue:** A compromised drand endpoint can return a valid BLS signature for a DIFFERENT (older) round than requested. Pairing check passes (valid for that older round), consumer receives 32 bytes of randomness they THINK is current. Exploitable if `is_round_recent` window > one drand period (3s) — which is the default.
**Fix:** one line — `if (BigInt(data.round) !== targetRound) continue;` after `response.ok` in the success branch.

### T1-03 — `hexToBytes` silent corruption on malformed input
**File:** `sdk/typescript/src/drand.ts:49-55`
**Surfaced by:** Lin (T1-B), Raul (T2), Fen (T3)
**Issue:** Odd-length hex → silent last-nibble truncation. Non-hex chars (`"zz"`, unicode) → silent zero bytes via `parseInt` NaN coercion. Consumer gets wrong bytes sent on-chain with no exception. A MITM'd drand endpoint returning `signature: "🔥..."` produces an all-zeros signature passed to verify → CU burn for error 6001 with no clear root cause.
**Fix:** validate `/^[0-9a-f]{128}$/i.test(hex)` (exactly 64 bytes = 128 hex chars for drand sigs) before conversion. Throw `AleaError` with clear message on mismatch.

### T1-04 — `createVerifyInstruction` throws unguarded RangeError on bad round
**File:** `sdk/typescript/src/instruction.ts`
**Surfaced by:** Lin (T1-A)
**Issue:** `roundBuf.writeBigUInt64LE(-1n)` or `writeBigUInt64LE(u64::MAX + 1n)` throws a Node-internal `RangeError` with opaque message. Consumer passing user-supplied round values without pre-validation gets uncaught internal error.
**Fix:** guard `if (round < 0n || round > 18446744073709551615n) throw new AleaError(6002, "round must be in [1, u64::MAX]")` before the buffer write. (Also handles round=0 via round < 1n variant.)

### T1-05 — `verifyDrandBeacon` null-signer TypeError
**File:** `sdk/typescript/src/client.ts`
**Surfaced by:** Lin (T1-C)
**Issue:** `isBrowserWallet(null)` → `"sendTransaction" in null` → uncaught `TypeError: Cannot use 'in' operator to search for 'sendTransaction' in null`. No null guard on `signer`.
**Fix:** `if (!args.signer) throw new AleaError(... "signer is required")` at function entry.

### T1-06 — `verifyDrandBeacon` no signature length validation
**File:** `sdk/typescript/src/client.ts`
**Surfaced by:** Lin (T1-D)
**Issue:** `args.signature` passed directly to Anchor via `Array.from(args.signature)` with no length assertion. 0-byte, 32-byte, or 1M-byte silently build malformed tx. 1M-byte case creates >1MB tx buffer failing at `sendRawTransaction` with cryptic packet error.
**Fix:** `if (args.signature.length !== 64) throw new AleaError(6001, "signature must be exactly 64 bytes (G1 uncompressed x||y)")` at entry.

### T1-07 — `package.json`: `peerDependenciesMeta` without `peerDependencies` key
**File:** `sdk/typescript/package.json`
**Surfaced by:** Nia (T1-A)
**Issue:** `peerDependenciesMeta` entry for `@solana/wallet-adapter-base` exists but there's no `peerDependencies` key. npm/pnpm/yarn silently ignore orphan meta → consumers get zero install-time signal about wallet-adapter compat.
**Fix:** add `peerDependencies: { "@solana/wallet-adapter-base": "^0.9.0" }` (or whatever range). Or remove the meta entry entirely if wallet-adapter isn't actually a peer dep.

### T1-08 — `cpi::verify` accepts raw AccountInfo without owner check (non-Anchor escape)
**File:** `sdk/rust/src/cpi.rs:58`
**Surfaced by:** Dmitri (T1)
**Issue:** `cpi::verify` takes raw `AccountInfo` args. `seeds::program` enforcement lives only in the consumer's `#[derive(Accounts)]`. A consumer calling from a non-Anchor path (raw `process_instruction`, governance relay, CPI forwarder) bypasses the mandatory constraint — attacker can substitute fake Config PDA with their own `pubkey_g2`.
**Fix options (TASTE DECISION — surface to Aaron):**
- (a) Add `require!(config.owner == &crate::PROGRAM_ID, ...)` inside `cpi::verify` (~200 CU)
- (b) Change signature from `AccountInfo` to `Account<'info, Config>` — Anchor enforces at type level
- (c) Keep as-is, add loud docs "Anchor-only, non-Anchor callers must self-enforce"
**Recommendation:** (a) owner check — costs ~200 CU (0.04% of 900K budget), closes a real footgun, defense in depth.

### T1-09 — README Rust quick-start: `AleaVerify` typo (doesn't exist)
**File:** `sdk/rust/README.md:36`
**Surfaced by:** Maya (T3→T1 dedup), Dmitri (T3), Ben (T1-1)
**Issue:** Quick-start imports `AleaVerify` (no `r`). Actual public export is `AleaVerifier`. Copy-paste → immediate E0425 compile error. First thing new consumer hits.
**Fix:** README line 36: `use alea_sdk::{self, AleaVerify};` → `use alea_sdk::{self, AleaVerifier};`.

### T1-10 — README Rust quick-start: `player_a`, `player_b` undefined
**File:** `sdk/rust/README.md:89`
**Surfaced by:** Maya (T1-MC-01), Ben (T1-4)
**Issue:** Quick-start code block uses `player_a` and `player_b` identifiers that are never declared. Code block lacks `rust,ignore` annotation → `cargo test --doc` fails AND consumer copy-paste fails.
**Fix:** Either add `rust,ignore` to the fence OR rewrite the snippet with concrete declarations (e.g., `let player_a = Pubkey::new_unique(); let player_b = ...;`).

### T1-11 — README error code table has fabricated variant names
**File:** `sdk/rust/README.md:179-184`
**Surfaced by:** Ben (T1-2), Priya (T3), Alicia (indirectly)
**Issue:** Error table labels:
- 6003 as `ChainHashMismatch` → actual `InvalidFieldElement`
- 6005 as `InvalidChainHash` → actual `InvalidG2Point`
- 6007 as `InvalidPubkeyG2` → actual `WrongChainHash`
- 6008 as `InvalidPublicKey` → actual `WrongPubkey`
Integrators matching on wrong codes in prod → silent bugs + broken retry loops.
**Fix:** regenerate table from `programs/alea-verifier/src/errors.rs` enum. Add CI check (per Phase 7 plan).

### T1-12 — README TS quick-start: "programId defaults to mainnet" comment wrong
**File:** `sdk/typescript/README.md` or root `README.md:87`
**Surfaced by:** Alicia (T1)
**Issue:** Comment says programId defaults to mainnet. Actual default is `DEVNET_PROGRAM_ID`. Consumers silently ship prod to devnet.
**Fix:** s/mainnet/devnet/ in the comment. Add prominent banner: "v0.1.0 ships DEVNET as default program ID. Mainnet pending Phase 5."

### T1-13 — README TS quick-start: documents removed `commitment` option
**File:** `sdk/typescript/README.md:53-58`
**Surfaced by:** Alicia (T1), Kai (T3)
**Issue:** Leftover from my Phase 1 A2 decision to remove dead `commitment?: Commitment` param. README still documents it. Consumer writes code against documented API → TS compile fails.
**Fix:** Remove `commitment?` from README API reference. (My Phase 1 commit removed it from code but missed the README.)

### T1-14 — `cpi` feature flag is a no-op
**File:** `sdk/rust/Cargo.toml:20,28`
**Surfaced by:** Maya (T1-MC-02)
**Issue:** `[features] cpi = ["alea-verifier/cpi"]` but `alea-verifier = { ..., features = ["cpi"] }` is unconditional in dependencies → the SDK's `cpi` feature does nothing. Any consumer project topology treating it as opt-in gets silently over-compiled now, and breaks if the dep line were ever corrected.
**Fix options (TASTE DECISION — surface to Aaron):**
- (a) Remove the SDK's `cpi` feature entirely (always-on) — simplest
- (b) Make it actually conditional: `dep = { ..., features = [] }` in deps, feature adds `["cpi"]`
- (c) Keep both, document that SDK's `cpi` feature is a stable alias for downstream convenience
**Recommendation:** (a) remove. The feature isn't doing anything; removing is cleaner than making it do something.

### T1-15 — Workspace `Cargo.toml`: `[patch.crates-io]` section structurally present (commented)
**File:** `Cargo.toml:22-24`
**Surfaced by:** Ben (T1-3)
**Issue:** `[patch.crates-io]` table header exists with commented-out content. Any future uncomment poisons downstream consumer lockfiles + signals fragile dep posture to grant reviewers.
**Fix:** delete the whole `[patch.crates-io]` block (lines 22-24). If we ever need it again, re-add at that point.

### T1-16 — example-lottery: `close = player` + direct lamport manipulation underflow risk
**File:** `programs/example-lottery/src/lib.rs:150-167` and `:225` (`close = player` constraint)
**Surfaced by:** Dmitri (T1)
**Issue:** `resolve_bet` subtracts lamports via `try_borrow_mut_lamports()` AND uses `close = player` constraint. `close` expects to move `bet.lamports()` at instruction end; if handler already drained `amount`, `close`'s subtract can underflow on older SBF toolchains. Consumers copying the "canonical" example get a latent funds-at-risk pattern.
**Fix options (TASTE DECISION — surface to Aaron):**
- (a) Rewrite payout to use `system_program::transfer` with PDA signer, remove direct borrow
- (b) Remove `close = player`, close manually after the transfer
- (c) Keep both but add invariant comment + `debug_assert!(bet.lamports() >= amount)` — risky
**Recommendation:** (a) — most idiomatic Anchor pattern for this use case. Small rewrite.

## T2 findings (should fix before publish for DX/hygiene)

Grouped by theme. Details in individual audit files.

### Security hardening (narrow safety margins)
- **T2-01** `sdk/rust/src/lib.rs:162` — `clock.unix_timestamp as u64` silent negative cast. Fix: `.max(0) as u64`. (Dmitri, Priya, Maya)
- **T2-02** Future-round accept in `is_round_recent` — intentional per my A1 decision aligning TS to Rust (Priya flags as narrower anti-replay margin). **Already decided — keep as-is.** Document the trade-off in README.
- **T2-03** `BeaconVerified.payer` event logs end-user wallet. Privacy-sensitive consumers should route through program PDA. (Priya) Add a `#[doc(warning)]` in lib.rs + explicit callout in README.
- **T2-04** `config.pubkey_g2` trusted post-initialize; no defense-in-depth re-check in `verify`. Adds ~200 CU. (Priya)

### TypeScript SDK input/response hardening
- **T2-05** `fetchBeacon` no response size cap — OOM attack via large body. Fix: check Content-Length ≤ 4KB. (Fen)
- **T2-06** `fetch` follows redirects unrestricted — MITM risk. Fix: `redirect: "error"`. (Fen)
- **T2-07** `AleaError(0, "...")` on drand exhaustion — code 0 absent from ERRORS map. Fix: define error code 6100 for "all endpoints failed" or similar. (Alicia, Fen)
- **T2-08** `MAINNET_PROGRAM_ID.toString()` returns undefined (proxy carve-out too broad). Should throw on `.toString()` too. (Lin, Nia)

### TypeScript SDK package hygiene
- **T2-09** Duplicate IDL in tarball: `src/idl/**` + `dist/**` both shipped. 9.1KB deadweight. Fix: remove `src/idl/**` from `files` array. (Nia, Alicia)
- **T2-10** `@solana/web3.js` + `@coral-xyz/anchor` are hard `dependencies`, should likely be `peerDependencies`. Prevents double-copy in consumer bundles, avoids class-identity issues. BUT this is a BREAKING CHANGE in some deployment shapes — surface to Aaron. (Alicia)
- **T2-11** `moduleResolution: "bundler"` in tsconfig — may not be ideal for published libraries. `"node16"` or `"nodenext"` is more conservative. (Nia)
- **T2-12** `MAINNET_PROGRAM_ID` typed as `PublicKey` in `.d.ts` but throws at runtime — type lie. Fix: type as `PublicKey | never` or add JSDoc warning. (Nia, Alicia)
- **T2-13** vitest pinned to EOL 1.x — upgrade to 2.x. (Nia)
- **T2-14** Missing `"default"` export condition — some bundlers require it. (Nia)

### TypeScript SDK browser DX
- **T2-15** No `AbortSignal` threading through `fetchBeacon`/`getVerifiedRandomness`. User navigates away mid-verify → stale tx still fires. (Kai, Raul)
- **T2-16** `wallet.signTransaction` called without capability guard — hardware/watch wallets throw opaque errors. (Kai)
- **T2-17** `skipPreflight: true` unconditional — no debug opt-out for integrators. (Kai)
- **T2-18** Wallet type is `@coral-xyz/anchor.Wallet`, not wallet-adapter's `WalletContextState`. Most browser consumers will hit type mismatch. (Alicia)

### Rust SDK DX
- **T2-19** `cpi::verify` return missing `#[must_use]` — silent empty-randomness footgun if consumer ignores return value. (Marcus)
- **T2-20** CPI consumer's `min_allowed_round` formula in example-lottery adds extra period delay (Dmitri). Minor DX.
- **T2-21** `AleaError` 6003 `#[msg]` text contradicts "unreachable/reserved" semantics. Retry loops may misbehave. (Dmitri, Priya)
- **T2-22** `cpi.rs` no mention of Solana BPF rustc lag workaround (pin `constant_time_eq=0.4.2`) for external consumers. (Tier-3 session learning). Should be in README troubleshooting.

### Rust ecosystem hygiene
- **T2-23** `no-std` category incorrectly claimed in Cargo.toml (SDK uses std via Anchor). (Ben)
- **T2-24** `docs.rs` metadata block for `alea-verifier` missing `cpi` feature (SDK has it but program crate doesn't). (Ben)
- **T2-25** Missing `authors` field in both Cargo.toml files. (Ben)
- **T2-26** No `exclude` pattern to signal test-file inclusion intent. (Ben)

### Non-actionable (already decided or acknowledged)
- Hana's T2 (declarative macro for seeds::program helper) — Phase 5 external audit item
- Marcus's T2 (correlated randomness doc warning) — trivial README addition
- Priya T2 on payer aliasing — correct behavior, doc only

## T3 findings (polish backlog)

~25 T3 items. Not blocking publish. Captured in individual audit files for follow-up. Top picks for Phase 7 (DX polish):
- Stronger `isBrowserWallet` structural discriminant (Raul)
- Subpath export for `fetchBeacon`-only consumers (Kai)
- Rename G2 pubkey "Kyber byte ordering" comment to "EIP-197" (Dmitri)
- IDL freeze attestation in release flow (Raul)
- `sideEffects: false` in package.json for tree-shaking (Nia)

## Taste-decisions needing Aaron input (5 questions)

1. **T1-01 browser bundler strategy** — inline IDL / package.json browser field / conditional import / factory function
2. **T1-08 cpi::verify owner check** — add inline check / change to Account<Config> type / doc-only
3. **T1-14 cpi feature flag** — remove entirely / make properly conditional / document as alias
4. **T1-16 example-lottery payout rewrite** — system_program transfer / remove close / keep+harden
5. **T2-10 TS deps migration** — keep as hard deps / move to peerDeps (breaking shape change)

Plus mega-batch of T2s to approve/defer.

## Summary

| Tier | Count | Publish impact |
|---|---|---|
| T1 | 16 | ALL block publish (doc-coherence, validation gaps, bundler breakage) |
| T2 | ~28 | Should fix; some require taste decision |
| T3 | ~25 | Polish backlog, defer |
| Exploitable crypto | 0 | None — security posture strong |
| Replay/griefing | 0 | All structural guards held |

Next step: Aaron decision walk on T1 taste items + T2 prioritization. Then sequential fix pass (Phase 5).
