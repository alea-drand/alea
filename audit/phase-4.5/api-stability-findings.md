# Phase 4.5 — API Stability Final Review Findings

**Date:** 2026-04-19
**Branch:** feature/phase-4.5-audit
**Reviewer:** Claude Opus 4.7 (direct read of all public items)
**Freeze context:** ADR 0028 locks v1 on first publish. Last chance to change names/signatures/types before they're immutable.

## Scope reviewed

- `sdk/rust/src/{lib,cpi,accounts,errors}.rs` — every `pub` item
- `sdk/typescript/src/{index,client,instruction,drand,constants,errors,types}.ts` — every export
- `programs/alea-verifier/src/lib.rs` — `verify` instruction, `Config`, events, `AleaError`

## Mechanical metadata gaps (no decision needed — applying directly)

| ID | File | Issue | Fix |
|---|---|---|---|
| M1 | `sdk/rust/Cargo.toml` | Missing `rust-version` | Add `rust-version = "1.79"` (matches release.yml pin) |
| M2 | `programs/alea-verifier/Cargo.toml` | Missing `rust-version` | Add `rust-version = "1.79"` |
| M3 | `sdk/rust/Cargo.toml` | `alea-verifier = { path = "..." }` won't publish | Add `version = "=0.1.0"` alongside `path` |
| M4 | `sdk/typescript/package.json` | Missing `homepage`, `bugs`, `engines` | Add `homepage: "https://alea.so"`, `bugs: { url: ".../issues" }`, `engines: { node: ">=18" }` |
| M5 | `.github/workflows/release.yml` | `toolchain: "1.79.0"` has TBD comment | Remove TBD comment — 1.79.0 is final |

## API-level findings (Aaron decision required)

### A1 — TS ↔ Rust `isRoundRecent` behavior divergence on future rounds

**Observation:** Rust and TS implementations diverge when the round is in the future (roundTs > currentTs).

Rust (`sdk/rust/src/lib.rs:158-164`):
```rust
current_timestamp.saturating_sub(round_timestamp) <= max_age_seconds
// Future rounds: saturating_sub returns 0 → 0 <= max_age → TRUE (recent)
```

TS (`sdk/typescript/src/drand.ts:31-41`):
```typescript
const age = clock.unixTimestamp - roundTs;
return age >= 0n && age <= maxAgeSeconds;
// Future rounds: age < 0 → FALSE (not recent)
```

Two SDKs implementing the same semantic check differ on future-round behavior. Both are defensible individually; cross-SDK inconsistency is the problem.

**Options:**
- **A1a (recommended):** Align TS to Rust — future rounds return `true` (they'll be "recent enough" by commit time). Matches the on-chain semantics since that's what the consumer's Rust program would see.
- **A1b:** Align Rust to TS — future rounds return `false` (paranoid). Requires changing Rust to explicit signed math.
- **A1c:** Document divergence + add warning in both, but keep both behaviors. Worst option (invisible footgun).

### A2 — Dead `commitment?` parameter in `getVerifiedRandomness`

**Observation:** `getVerifiedRandomness` accepts `commitment?: Commitment` but never uses it (`verifyDrandBeacon` hardcodes `commitment: "confirmed"` in the AnchorProvider).

**Options:**
- **A2a (recommended):** Remove the parameter. It's a lie that it's configurable.
- **A2b:** Wire it through — pass `commitment` to AnchorProvider + confirmTransaction. Then it actually does what the type says.

### A3 — `createVerifyInstruction` footgun — no payer key

**Observation:** `createVerifyInstruction` returns a TransactionInstruction with only the config account in keys. The comment says "Callers must add the payer signer account to the returned instruction's keys before submitting." A caller who doesn't read that comment ships a tx that fails at runtime with a non-obvious error.

This is for the low-level "bring your own tx" use case (per `sdk/typescript.md` spec), but the ergonomics invite mistakes.

**Options:**
- **A3a (recommended):** Require `payer: PublicKey` as a param; add it to keys internally. Eliminates the footgun. Slight break from pure "instruction builder" idiom but this is v0.1.0 so nothing to break yet.
- **A3b:** Keep as-is, add a loud doc warning + TypeScript narrowing to enforce the caller adds keys. More complex.
- **A3c:** Remove `createVerifyInstruction` entirely — `verifyDrandBeacon` covers the 95% case; users who want raw Solana control can copy the 20 lines themselves. Trims the public API.

### A4 — `AleaVerify<'info>` Rust struct — may be unused dead weight

**Observation:** `AleaVerify<'info>` is exported as a "convenience Accounts fragment" but `programs/example-lottery/src/lib.rs` and the `sdk/rust/src/lib.rs` doc example both write out the full inline accounts struct instead of embedding `AleaVerify`. If the canonical example doesn't use it, does anyone?

**Options:**
- **A4a:** Keep as-is — optional helper for users who want it. Low cost to export.
- **A4b (recommended):** Remove. The doc example already shows users writing the constraints inline (that's the idiomatic Anchor pattern). Shipping unused API surface is clutter.
- **A4c:** Keep but update doc example to demonstrate `AleaVerify` usage, so there's a real pattern for users to copy.

### A5 — Dead TS types `VerifyOptions` and `BeaconResult`

**Observation:**
- `VerifyOptions` is exported but no function uses it — `verifyDrandBeacon` and `getVerifiedRandomness` inline their own options shapes.
- `BeaconResult` is exported but no function returns it — `fetchBeacon` returns `DrandBeacon`.

Both are dead types in the public API. ADR 0028 freezes them at v1.

**Options:**
- **A5a (recommended):** Remove both before publish. Zero consumer impact (nothing uses them).
- **A5b:** Refactor `verifyDrandBeacon`/`getVerifiedRandomness` to actually use `VerifyOptions` for consistency. Merge `BeaconResult` into `DrandBeacon` or delete.

### A6 — `ERRORS` is mutable — consumers could mutate it

**Observation:** `export const ERRORS: Record<number, string> = { ... }` — const binding but the object's keys are writable. A misbehaving consumer (or attacker via a dep) could `ERRORS[6000] = "something misleading"` in their process.

**Options:**
- **A6a (recommended):** `Readonly<Record<number, string>>` + `as const` on the object literal. Compile-time immutability.
- **A6b:** Wrap with `Object.freeze()` at module init. Runtime immutability.
- **A6c:** Both (defensive).

## Findings I'm NOT flagging (kept as-is after review)

- `config_pda` returns `(Pubkey, u8)` — tuple is the Anchor idiom
- `PROGRAM_ID` const — correct shape
- `cpi::verify` 5-arg positional signature — deliberate, documented
- `MAINNET_PROGRAM_ID` throw-proxy — Aaron-confirmed 2026-04-19
- `DRAND_*` constants as flat exports — correct per earlier decision
- `isBrowserWallet` structural discriminant — works
- `SolanaClock` / `DrandConfig` interfaces — used in `isRoundRecent`, correctly shaped
- `fetchBeacon` 3×5 retry behavior — documented, T2.01 spec
- Error code range 6000-6009 — frozen per ADR 0028 append-only
- `AleaError` class shape (`code` + `message`) — minimal, stable

## Summary

- **6 mechanical fixes** — applying directly, no interview needed
- **6 API-level findings** — all require Aaron decision before persona audit locks them in
- **Multiple items validated as keep-as-is** — freeze test passed

Next step: interview Aaron on A1–A6 via AskUserQuestion, apply decisions, commit.
