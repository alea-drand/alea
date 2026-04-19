# Rust SDK Re-Verification — alea-sdk v0.1.0

**Auditor persona:** Jaiden Okafor — senior backend engineer (12 yr Go, 3 yr Rust in backend services; first-time Solana program author). Evaluating for a gamified savings product with daily winner selection via verified randomness.

**Date:** 2026-04-19
**Branch:** feature/phase-4-sdk
**Test environment:** `/tmp/alea-rust-verify-2/savings-lottery/`
**Prior findings reviewed:** Phase 4.5 consolidated (16 T1s, 28 T2s); focus items T1-17/18 and T2-19.
**Methodology:** Follow README cold. `cargo check` from scratch. Intentional break tests. `cargo doc`. Pin removal test. Scorecard against backend-Rust-dev bar.

---

## Scorecard

| Axis | Score | Evidence |
|------|-------|----------|
| **First compile success** | 9/10 | `cargo check` passed on first attempt following README literally. One required step not in the "Install" section: `constant_time_eq = "=0.4.2"` pin (documented in Troubleshooting). Drop to 8 if user reads Install only; 9 if they read Troubleshooting. |
| **API clarity** | 9/10 | `cpi::verify(program, config, payer, round, sig)?` is flat, zero boilerplate. `.into_inner()` and `.as_bytes()` are idiomatic Rust. `VerifiedRandomness` wrapper is cleanly designed. Better than most Solana SDK APIs I've seen. Minor: `VerifiedRandomness` not constructable by consumers for unit tests — requires test fixture workaround. |
| **Documentation completeness** | 8/10 | README covers all the important things. `lib.rs` doc example matches `README.md` Quick Start exactly. `cargo doc -p alea-sdk --no-deps` generates clean HTML with zero warnings. Gaps: (1) `constant_time_eq` pin applies to stable rustc 1.94, not just BPF — Troubleshooting says "cargo build-sbf" but fails on plain `cargo check` too; (2) no minimal standalone compilable example (rust,ignore + external link). |
| **Error-guidance quality** | 7/10 | The `declare_id!` cascade is real and documented correctly in README Troubleshooting. Verified live: bad 32-char placeholder → "pubkey array is not 32 bytes long" + cascade `E0425: cannot find value 'ID'` with misdirected import hint. README warns about this but a consumer scanning quickly will waste 5-10 minutes. `must_use` violation warning message is excellent ("Alea's return data is single-slot — capture randomness into a local before any other CPI"). |
| **Security rigor** | 9/10 | `seeds::program = alea_program.key()` is MANDATORY-capitalized in every location it appears — README, lib.rs, cpi.rs, the test consumer I wrote. `is_round_recent` required status is unmissable. Return-data ordering `// WRONG` vs `// CORRECT` block is concrete and accurate. `VerifiedRandomness` newtype catches the forgotten-capture bug at compile time (confirmed). Defense-in-depth owner check in `cpi::verify` for non-Anchor callers (T1-08) is present. |
| **First impression** | 8/10 | This feels like Rust, not "Solana-Rust". Flat function args instead of nested builders. No callback ceremony. The CPI wrapper does what it says on the tin. The `#[must_use]` wrapper is the kind of thing a careful library author adds — I noticed and appreciated it. The Pin requirement is annoying but the error is clear. |

**Overall: 8.3/10**

---

## Prior T1 Fix Verification

### T1-17 — `#[must_use]` gap (originally T2-19 in consolidated)

**Status: FIXED and confirmed working.**

The fix wraps the return in a `VerifiedRandomness([u8; 32])` struct with `#[must_use]` at the struct level. I verified two scenarios:

1. **Correct pattern** (`let randomness = cpi::verify(...)?`**.**`into_inner()`) — compiles clean, no warnings.

2. **Intentional break** — simulated `simulated_verify()?;` without binding on a `#[must_use]` struct. Compiler output:

   ```
   warning: unused `VerifiedRandomness` that must be used
     --> must_use_test.rs:22:5
      |
   22 |     simulated_verify()?;
      |     ^^^^^^^^^^^^^^^^^^^
      |
      = note: Alea's return data is single-slot — capture randomness into a local before any other CPI
   ```

   The warning fires with the full custom message. The key insight: `#[must_use]` on the *function* would NOT fire here because `?` extracts the `Ok(VerifiedRandomness)` before Rust can flag it — the function's must_use only fires when the entire `Result` is dropped. Attaching `#[must_use]` to the *struct* catches the value after `?` extraction. This is the correct fix.

**Verdict: T1-17 is genuinely closed. The fix works at the level it needs to work.**

### T1-18 — `constant_time_eq` pin applies to native cargo, not just BPF

**Status: FIXED in README — partially. One remaining gap.**

The README Troubleshooting section now describes the pin correctly and says it's required for both contexts. I confirmed by removing the pin and running `cargo check`:

```
error: rustc 1.94.1 is not supported by the following package:
  constant_time_eq@0.4.3 requires rustc 1.95.0
Either upgrade rustc or select compatible dependency versions with
`cargo update <name>@<current-ver> --precise <compatible-ver>`
```

The error message is clear and rustc itself provides the fix command. With the pin in place, `cargo check` passes cleanly.

**Remaining gap (T2):** The README Install section says:
```toml
[dependencies]
alea-sdk = "0.1"
```
No mention of the pin here. A developer who reads Install → Quick Start → copy-pastes → `cargo check` will hit the failure before reaching Troubleshooting. The pin should appear as a companion in the Install section, not only in Troubleshooting.

**Verdict: T1-18 is functionally closed (docs are correct). The Troubleshooting placement creates friction; see new finding NF-1 below.**

### T2-19 (consolidated) → now T1-17 — the `#[must_use]` wrapper

Confirmed same as T1-17 above. The VerifiedRandomness wrapper also provides `.as_bytes()` for borrow-without-consume. Both methods are documented in the struct's rustdoc. The `From<VerifiedRandomness> for [u8; 32]` impl enables `.into()` as a third path. API surface is complete.

---

## Did-Fixes-Work Assessment

| Finding | Pre-fix behavior | Post-fix behavior | Verdict |
|---------|-----------------|-------------------|---------|
| T1-17 `#[must_use]` on return type | `cpi::verify()?;` silently dropped randomness with zero warning | Compiler warns with full message on `VerifiedRandomness` drop | CLOSED |
| T1-18 `constant_time_eq` pin scope | README mentioned BPF only; native cargo failure was undocumented | README Troubleshooting correctly describes both contexts | CLOSED (with NF-1 gap) |
| T1-08 non-Anchor owner check | `cpi::verify` accepted any AccountInfo for config | `require_keys_eq!(*config.owner, PROGRAM_ID)` in `cpi::verify` | CLOSED — confirmed in cpi.rs line 122 |
| T2-01 negative unix_timestamp | `clock.unix_timestamp as u64` silently wrapped | `.max(0) as u64` with doc comment | CLOSED — confirmed in lib.rs line 169 |

---

## New Findings

### NF-1 (T2) — `constant_time_eq` pin absent from Install section, only in Troubleshooting

**Severity:** T2 (DX friction, not a bug — the fix is documented)
**File:** `sdk/rust/README.md` — Install section
**Observed behavior:** A developer following Install → Quick Start → `cargo check` will fail with the constant_time_eq error before they reach Troubleshooting. The fix is there, but the placement is reactive rather than proactive.
**Fix:** Add the pin as a co-requirement in the Install section:
```toml
[dependencies]
alea-sdk = "0.1"
# Required: pin this transitive for rustc < 1.95 (stable as of 2026-04-19)
constant_time_eq = "=0.4.2"
```
**Effort:** 2-line README edit.

---

### NF-2 (T3) — `declare_id!` error cascade is documented but still confusing in 2-error form

**Severity:** T3 (known, documented, Anchor issue not SDK issue)
**Confirmed:** the cascade fires when `declare_id!` fails AND something else references `ID` in the same crate:
```
error: pubkey array is not 32 bytes long: len=24
error[E0425]: cannot find value `ID` in the crate root
  help: consider importing this constant
    |
  1 + use anchor_lang::system_program::ID;
```
The second error's "help" suggestion actively misdirects the developer. The README Troubleshooting explains this correctly. I would add one sentence: "Ignore the `E0425: cannot find value 'ID'` error and the import suggestion — it's a cascade from the first error."
**Effort:** 1 sentence in README Troubleshooting section.

---

### NF-3 (T3) — `VerifiedRandomness` not constructable in consumer unit tests

**Severity:** T3 (DX limitation, not a bug)
**Observed:** When writing a unit test that wants to test lottery logic against a known randomness value, I can't construct `VerifiedRandomness` — its inner field is private and there's no `pub fn new(bytes: [u8; 32]) -> Self` constructor. This is intentional (prevents misuse) but means consumer tests must either call the real CPI (requires BPF runtime) or use raw `[u8; 32]` directly in their math logic.
**In practice:** My workaround was to extract the winner-selection math into a separate function taking `[u8; 32]` directly, which is actually the better design. The SDK's API correctly forces this pattern.
**Verdict:** Acceptable design. Could add a `cfg(test)` constructor or a `#[doc(hidden)] pub fn new_for_test(bytes: [u8; 32]) -> Self` if consumer DX feedback supports it in Phase 5.

---

## Compile Verification Summary

| Check | Result |
|-------|--------|
| `cargo check` (savings-lottery with pin) | PASS — clean, no warnings |
| `cargo check` (savings-lottery without pin) | FAIL — expected, documented |
| `cargo test` | 3/3 PASS — `test_id`, `winner_selection_in_bounds`, `round_recent_boundaries` |
| `cargo doc -p alea-sdk --no-deps` | PASS — 0 warnings, clean HTML |
| `declare_id!` cascade (intentional break) | Reproduces exactly as documented in README |
| `#[must_use]` forgotten capture (intentional break) | Warning fires with correct custom message |
| `constant_time_eq` pin removal | Fails loudly and clearly with actionable error |

---

## Publish-Ready Verdict

**YES — ship v0.1.0 with the following notes.**

The security posture is strong. The two integration T1s (T1-17 must_use wrapper, T1-18 pin scope) are closed. The mandatory constraints (`seeds::program`, `is_round_recent`) are genuinely unmissable. `cargo doc` is clean. The API is more idiomatic Rust than anything in the broader Solana SDK ecosystem.

**What to fix before tagging the release (30 min of work):**
1. NF-1: add `constant_time_eq` pin to Install section (2 lines)
2. NF-2: add one clarifying sentence to the `declare_id` Troubleshooting entry (optional but worth it)

**What to accept and document:**
- NF-3 (VerifiedRandomness unit-test constructor): acceptable design, revisit in Phase 5 based on real consumer feedback
- The `declare_id!` UX issue is an Anchor bug, not fixable in the SDK

**Skeptical-backend-dev bar:** Would I recommend this to a colleague building a savings product? Yes. The API is clean, the docs are honest about maturity (CAVEATS.md is good), and the security guardrails are designed to be hard to miss rather than easy to skip. The `VerifiedRandomness` wrapper is the kind of defensive API design that distinguishes a library built by people who have thought about failure modes from one that hasn't.

The only thing I'd flag to a team lead: this is devnet-only, no external audit yet. The CAVEATS.md is clear about this. Don't ship to mainnet until Phase 5 gates close.

---

## Test Artifacts

- Consumer program: `/tmp/alea-rust-verify-2/savings-lottery/src/lib.rs`
- must_use break test: `/tmp/alea-rust-verify-2/must_use_test.rs`
- Bad declare_id test: `/tmp/alea-rust-verify-2/bad_declare_id_test/src/lib.rs`
- Cargo.toml with pin: `/tmp/alea-rust-verify-2/savings-lottery/Cargo.toml`
