# Rust SDK Integration Audit — alea-sdk v0.1.0

**Auditor persona:** Priya (senior Rust/Solana dev, first-time alea-sdk consumer)
**Date:** 2026-04-19
**Branch:** feature/phase-4-sdk
**Test environment:** `/tmp/alea-consumer-rust-audit/my-lottery/`
**Methodology:** Follow README Quick Start literally; attempt `cargo check`, `cargo test`, `cargo doc`; exercise `config_pda()`; probe security guardrails.

---

## Scorecard

| Axis | Score | Rationale |
|------|-------|-----------|
| **Installability** | 7/10 | `cargo check` succeeded after ONE fix (declare_id! padding). No mystery errors, no wrong dependency versions. The constant_time_eq pin was already documented and required no debugging. `cargo doc` required pinning constant_time_eq too — not mentioned in README for non-BPF builds. |
| **API ergonomics** | 9/10 | `cpi::verify()` signature is clean flat args (no context builder). `is_round_recent()` naming is self-documenting. `Config` and `AleaVerifier` are where you expect them. `config_pda()` works out of the box. `PROGRAM_ID` constant is obvious. |
| **Documentation clarity** | 8/10 | README + rustdoc are comprehensive. Security constraints are unmissable (bolded, repeated in `README.md`, `lib.rs`, `cpi.rs`). Gap: README's Quick Start example uses `rust,ignore` annotation with a comment about `YourGameState` but doesn't show a complete minimal example that actually compiles. Compute budget section is buried after the code example. |
| **Error quality** | 6/10 | No runtime errors hit. Two compiler friction points: (1) `declare_id!` macro has an opaque "pubkey array is not 32 bytes long: len=N" error with no hint that the issue is base58-encoding length — easy to get wrong with placeholder IDs. (2) When `declare_id!` fails, the cascade produces misleading `E0425: cannot find value 'ID' in this scope` with an unhelpful suggestion to import `anchor_lang::system_program::ID` — misdirects the user entirely. Both are Anchor bugs, not SDK bugs, but a consumer will blame the SDK. |
| **Completeness** | 7/10 | Missing: (1) No `cargo-build-sbf` instruction in README for consumers who aren't running anchor CLI — only mentioned in troubleshooting. (2) No minimal standalone consumer example that compiles (the `example-lottery` is referenced as a GitHub link, which returns 404 for private repos). (3) `errors` module is a re-export-only module with no items beyond `AleaError` — fine, but the doc page is thin. (4) No mention of `System` program or `system_program` account for consumers that need SOL transfers alongside Alea CPI. |
| **Security guidance** | 9/10 | Exceptional: `seeds::program = alea_program.key()` has MANDATORY capitalized in every context it appears, repeated in README security section, rustdoc lib.rs, and cpi.rs module doc. `is_round_recent` required status is clear. Return-data ordering warning in correct/wrong code block is effective. One gap: the `#[must_use]` on `cpi::verify` does NOT fire when consumers use the idiomatic `alea_sdk::cpi::verify(...)?;` pattern without binding (the `?` consumes the Result for error propagation but silently drops the randomness bytes). |

**Overall: 7.7/10**

---

## Ordered List of Frictions / Confusions

### F1 — `declare_id!` byte-length error with cascading misleads (HIGH IMPACT)

**What happened:** Using a placeholder program ID `"MyLottery111111111111111111111111111111111"` (41 chars) produced:
```
error: pubkey array is not 32 bytes long: len=31
```
followed by three cascading `E0425: cannot find value 'ID' in this scope` errors suggesting to import `anchor_lang::system_program::ID`.

**Why it's a problem:** The root cause message ("pubkey array not 32 bytes") doesn't explain that base58-encoded 32-byte pubkeys are typically 44 characters. A new consumer counting "1" characters by hand will get this wrong repeatedly. The cascade errors are completely misleading — they point toward a wrong import, not the actual problem. This hit me twice across two files during the audit.

**SDK responsibility?** The error is in Anchor's `declare_id!` macro, but the README should add a note: "Program IDs must be valid base58 pubkeys (44 chars for most keys; use `solana-keygen new` or `Pubkey::new_unique().to_string()` in tests)."

---

### F2 — `constant_time_eq` pin required for `cargo doc` (native toolchain), not just `cargo build-sbf`

**What happened:** Running `cargo doc` from a standalone SDK copy without adding `constant_time_eq = "=0.4.2"` immediately failed:
```
error: rustc 1.94.1 is not supported by the following package:
  constant_time_eq@0.4.3 requires rustc 1.95.0
```

**Why it's a problem:** The README troubleshooting section mentions this pin only in the context of `cargo build-sbf` (BPF toolchain). But the native `rustc 1.94.1` (stable at time of audit) also can't resolve `constant_time_eq 0.4.3`. Any consumer on rustc < 1.95 (which includes everyone on the current stable, 1.94.1) hits this with a regular `cargo check` or `cargo doc`.

**Fix:** README troubleshooting section should update the pin guidance to say: "Required for both `cargo build-sbf` AND `cargo check`/`cargo doc` on rustc < 1.95 (current stable as of 2026-04-19)."

---

### F3 — `#[must_use]` does not protect against the most common "forget to capture" pattern

**What happened:** Writing `alea_sdk::cpi::verify(...)?;` without binding the return produces ZERO compiler warnings, even though `#[must_use]` is on the function. The `?` operator consumes the `Result<[u8;32]>` for error propagation, silently discarding the `[u8;32]` success value. A tired developer who writes this misses the entire point of calling Alea.

**Why it's a problem:** The `#[must_use]` annotation string is carefully written: "Alea's return data is single-slot — capture randomness immediately; any later CPI overwrites it". But it only fires when the call's result is completely discarded (e.g., dropping the whole `Result` without `?`). The `?`-then-discard pattern — exactly what a Rust developer writes by reflex — bypasses it silently.

**Impact:** Security. A consumer can write this, ship it, and their program accepts the CPI fee but ignores the randomness entirely. The on-chain `BeaconVerified` event fires, the fees are paid, but the program uses `random_value = 0` or whatever its uninitialized state is.

**Fix options:**
- T1: Change return type to a newtype wrapper `struct Randomness([u8; 32])` with `#[must_use]` on the struct itself — Rust fires the warning on any `Randomness` value dropped without use, including after `?`. Cost: breaking change to the type, consumers must call `.0` or impl Deref.
- T2: Add an explicit clippy lint to the README: `#[warn(unused_must_use)]` / `cargo clippy -- -W unused-must-use`. Document the limitation in the cpi.rs doc comment.
- T3: Add to README security section: "**Important:** `alea_sdk::cpi::verify(...)?;` without binding the return is silently valid Rust. Always write `let randomness = alea_sdk::cpi::verify(...)?;`."

---

### F4 — README Quick Start example references a non-compiling hypothetical type

**What happened:** The code block uses `YourGameState` and annotates with `rust,ignore`. The comment says "see `programs/example-lottery/` in the repo for a complete, compiling reference consumer" with a GitHub link. That link will 404 for users who don't have repo access (crates.io consumers using `cargo add alea-sdk`).

**Why it's a problem:** First-time consumers following the README will see a non-compilable example as the primary reference. The annotated comment explains why, but a developer who's scanning quickly may miss it and be confused when the code doesn't work as-pasted.

**Fix:** Add a minimal, complete, self-contained example (with a real `declare_id!`, a stub `GameState` struct, and no external references) that can be `cargo check`-ed. The current example is ~80% of the way there — just needs a `GameState` stub and a valid ID.

---

### F5 — No mention of `cargo check` vs `cargo build-sbf` workflow

**What happened:** The README and troubleshooting go straight to `cargo build-sbf` failure modes. A consumer who wants to iterate fast with `cargo check` (the standard Rust workflow) has to figure out on their own that this is the right step. The troubleshooting section is titled around `cargo build-sbf` which makes it feel like that's the primary workflow.

**Fix:** Add a "Development Workflow" section above troubleshooting: "`cargo check` works for type-checking. Use `cargo build-sbf` only when you need to produce a deployable `.so` artifact."

---

### F6 — Compute Budget section buried after main code example

**What happened:** The 900K CU requirement is documented, but it appears after the code example and security constraints. A developer who integrates following the Quick Start and tests on a local validator may silently get "Program failed to complete" without knowing why — the SDK's TypeScript version handles this automatically, but the Rust SDK must add it manually.

**Why it matters:** The error message from Solana for compute budget exceeded is generic. The connection to the 900K requirement is not obvious.

**Fix (T2):** Move the compute budget warning to a `> [!warning]` callout at the TOP of the Quick Start, before the code example. Consider a link to Anchor's `ComputeBudgetInstruction` example.

---

### F7 — `errors` module documentation is a stub

**What happened:** The `alea_sdk::errors` module rustdoc page shows only one public item (`AleaError` re-export) with no module-level explanation of how error codes map to on-chain behavior for consumers.

**Why it matters:** A consumer catching errors from a failed CPI needs to know which errors are retryable vs permanent. The information exists in the README error table but is not linked from the rustdoc.

**Fix (T3):** Add a `//!` module doc to `src/errors.rs` explaining: "These error codes are emitted by the Alea on-chain program. Codes 6000-6012 are stable per ADR 0028. See the [error table in the README] for retryability guidance." Add an intra-doc link.

---

## What Worked Well

1. **Zero friction `cargo check` on first real attempt.** After the dummy-ID issue (which is an Anchor problem), the actual API compiles exactly as the README documents it. No version conflicts, no missing features, no surprising transitive dep issues.

2. **`config_pda()` helper is clean and discoverable.** The function exists where you'd look for it (`alea_sdk::config_pda`), has clear docs about the bump caching pattern, and works correctly. Tests pass immediately.

3. **Security constraints are genuinely impossible to miss.** Every occurrence of `alea_config` in the example code has `seeds::program = alea_program.key()` with the MANDATORY comment. The same warning appears in rustdoc lib.rs, README security section, and cpi.rs module doc. Four reinforcing mentions — this is better than most production SDKs.

4. **`is_round_recent()` naming is excellent.** A developer reading the Accounts struct sees `is_round_recent(round, &ctx.accounts.alea_config, &ctx.accounts.clock, 30)` and immediately understands what it does and what `30` means. No magic numbers without context.

5. **Return data ordering warning with CORRECT/WRONG side-by-side.** The explicit code block showing what breaks the data ordering invariant is highly effective. This is the kind of footgun that usually only shows up as a runtime bug — the SDK surfaces it at documentation time.

6. **`AleaError` variants are well-documented.** Each error code has a msg, a numeric code, and an in-source comment explaining when it fires and whether it's retryable. The README error table is complete and matches the source.

7. **`cargo doc` generates zero warnings, zero broken intra-doc links.** All public items have doc coverage. The crate-level `lib.rs` docs are comprehensive.

8. **`const PROGRAM_ID` is re-exported from the verifier, preventing drift.** This is a subtle but important correctness property — the SDK cannot have a different program ID than the on-chain program at compile time.

---

## Recommendations

### T1 — Critical (blocks confident publish)

**T1-01** — `sdk/rust/src/cpi.rs:48` / README Quick Start
**Problem:** `#[must_use]` on `cpi::verify` does not fire when consumer writes `alea_sdk::cpi::verify(...)?;` without binding. Silent security bug.
**Fix:** Change the return type to a newtype `pub struct Randomness(pub [u8; 32])` with `#[must_use]` on the struct. This propagates the warning through `?`. Alternatively: document the limitation explicitly in the security section as a code-pattern anti-example, and add to the README that this pattern is silently wrong.
**Breaking change:** Yes, if choosing the newtype approach (`randomness.0[0..8]` vs `randomness[0..8]`). Mitigatable with `impl Deref<Target = [u8; 32]>`.

**T1-02** — `sdk/rust/README.md` Troubleshooting section
**Problem:** `constant_time_eq = "=0.4.2"` pin is documented only for `cargo build-sbf`. It's also required for `cargo check` and `cargo doc` on rustc < 1.95 (current stable).
**Fix:** Update the troubleshooting header: "cargo build-sbf OR cargo check fails on `constant_time_eq@0.4.3 requires rustc 1.95`" and expand the explanation accordingly.

### T2 — Important (notable friction, should fix before stable release)

**T2-01** — `sdk/rust/README.md` Quick Start
**Problem:** The `declare_id!` placeholder in any example that doesn't compile will produce a confusing cascade of errors misdirecting developers to `anchor_lang::system_program::ID`. No guidance on how to construct a valid placeholder.
**Fix:** Add a comment under "Quick Start" installation: "For local testing, use `Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS` as a placeholder pubkey, or run `solana-keygen new` to generate one."

**T2-02** — `sdk/rust/README.md` Quick Start
**Problem:** Compute budget requirement (900K CU) is buried after the code example. Consumers testing on localnet will hit silent "Program failed to complete" without diagnosis path.
**Fix:** Move the Compute Budget section above the security constraints, and add a `> [!warning]` callout: "Every transaction calling Alea MUST set compute unit limit ≥ 900,000. Localnet and simulations may not enforce this — test with a real compute budget instruction."

**T2-03** — `sdk/rust/README.md` Quick Start
**Problem:** The code example is annotated `rust,ignore` and references `YourGameState`. The link to the full example points to GitHub (404 for crates.io consumers).
**Fix:** Add a second code block with a minimal but complete, self-contained example (add a stub `#[account] pub struct GameState {}` and use the real program ID placeholder).

**T2-04** — `sdk/rust/README.md` (new section)
**Problem:** No guidance on development workflow — when to use `cargo check` vs `cargo build-sbf`.
**Fix:** Add a "Development Workflow" section: "`cargo check` for fast iteration. `cargo build-sbf` to produce the deployable `.so`. `anchor build` is NOT supported with Anchor 0.30.1 + modern proc-macro2 (see Troubleshooting)."

### T3 — Polish (nice-to-have before stable release)

**T3-01** — `sdk/rust/src/errors.rs:1-7`
**Problem:** Module-level doc is thin — no explanation of retryability or link to the README error table.
**Fix:** Expand the `//!` module doc to include: which errors are retryable (none are — all are deterministic), which are "reserved/unreachable", and an intra-doc link to the README error table.

**T3-02** — `sdk/rust/README.md` (new example)
**Problem:** No example of how to handle `AleaError` codes in a consumer's error-handling code (e.g., matching on `AleaError::InvalidSignature`).
**Fix:** Add a brief Errors section showing how to detect and handle Alea CPI errors in an Anchor consumer's `match` expression.

**T3-03** — `sdk/rust/README.md` (new section)
**Problem:** No "Next Steps / Related Resources" at the bottom. Consumers who want more depth have nowhere to go except the CAVEATS.md link.
**Fix:** Add links to: CAVEATS.md (already referenced in warning at top, but easy to miss), the error table anchor, and the compute budget section.

---

## Summary: Ready to Publish?

**Verdict: NOT YET — 2 T1s must be resolved first.**

The SDK is technically solid. `cargo check` works, the API is clean, and the security documentation is genuinely excellent. But two issues require resolution before confident crates.io publish:

1. **T1-01 (silent security footgun):** `cpi::verify(...)?;` without binding produces no compiler warning. A fatigued developer ships a program that pays CPI fees but ignores randomness. The `#[must_use]` annotation as written provides a false sense of protection. This needs either a newtype return type or explicit anti-pattern documentation in the security section.

2. **T1-02 (rustc version range for the pin):** The `constant_time_eq` pin is documented only for BPF builds, but rustc 1.94 stable (current as of 2026-04-19) also needs it for regular `cargo check`. Any developer on stable will hit this unexpectedly.

After those two fixes, the T2 items (compute budget placement, self-contained example, declare_id guidance) would take the experience from "good" to "excellent." The underlying implementation quality is high — the T-count here reflects polish gaps, not correctness issues.
