# Persona Audit — Maya Chen, Senior Rust/Solana Dev
**Date:** 2026-04-19
**Scope:** External consumer evaluation of `alea-sdk` v0.1.0
**Files read:** `sdk/rust/` (full tree), `programs/alea-verifier/src/` (source), `programs/example-lottery/src/lib.rs`, root `README.md` + `CHANGELOG.md`
**Simulation:** First-contact from crates.io. No internal context. Evaluated as potential drop-in for ORAO VRF.

---

## Execution Notes

Cold-read sequence: README → sdk/rust/README.md → Cargo.toml → lib.rs → cpi.rs → errors.rs → example-lottery. Checked verifier source for types I'd instantiate. Total read time: ~18 min. Would have reached for crates.io, seen the `alea-sdk` page, landed on the GitHub README.

Overall impression: the security documentation is unusually thorough for a v0.1 crate — the mandatory-constraint warnings in README and lib.rs doc are better than most mature oracle SDKs. The example-lottery program is a genuine reference consumer, not toy code. The failure mode that nearly made me close the tab: a non-compiling snippet in the root README. The failure that made me slow down: feature-flag confusion in Cargo.toml.

---

## Findings

### T1 — Build Blockers

**[T1-MC-01] `sdk/rust/README.md:89` — Quick Start snippet references undefined identifiers `player_a` and `player_b`**

The README quick-start example assigns `let winner = if random_value % 2 == 0 { player_a } else { player_b };`. Neither variable is declared anywhere in the snippet. A developer who pastes this into their program gets an immediate compile error (`E0425: cannot find value player_a in this scope`). For a first-contact consumer this is the 60-second close-the-tab moment — the quickstart is the first code they try to compile. The `example-lottery/src/lib.rs` correctly declares `let player_wins = random_value % 2 == 0;` but the SDK README never catches up. Fix: replace `player_a`/`player_b` with `0u64`/`1u64` or a comment placeholder so the snippet is syntactically self-contained.

**[T1-MC-02] `sdk/rust/Cargo.toml:20,28` — `cpi` feature is declared but `alea-verifier` is always pulled with `features = ["cpi"]` unconditionally, making the feature flag a no-op that silently over-compiles for consumers who omit it**

`[features] cpi = ["alea-verifier/cpi"]` implies that omitting `features = ["cpi"]` in a downstream `Cargo.toml` skips the CPI bindings. But the `[dependencies]` block hard-codes `alea-verifier = { ..., features = ["cpi"] }` unconditionally. The `cpi` feature on `alea-sdk` itself therefore does nothing — the verifier CPI code is always compiled in. A consumer who tries `alea-sdk = { version = "0.1", default-features = false }` to avoid pulling in CPI machinery (e.g., a monitoring-only crate that only reads `Config`) will still pull it all in, and `alea_sdk::cpi` will always be available regardless of what they declare. Worse, a consumer who reads the feature table and infers "I need `features = [\"cpi\"]` to call verify" is correct by accident for the wrong reason — if the unconditional dep line were ever fixed, their code would break when they omit the feature. Fix: gate `alea-verifier` in `[dependencies]` behind the `cpi` feature using `optional = true` + `[features] cpi = ["dep:alea-verifier"]`.

---

### T2 — Should Fix

**[T2-MC-01] `sdk/rust/README.md:150-157` — Compute budget section says "Rust consumers must add it manually" but gives no account-type import path or full working snippet**

The CU section shows `ComputeBudgetInstruction::set_compute_unit_limit(900_000)` without a `use` statement or crate path. A consumer who hasn't worked with the compute budget instruction before has to go find `solana_sdk::compute_budget::ComputeBudgetInstruction` themselves — and the `solana-sdk` version pinned by `anchor-lang 0.30.1` (`1.18.26`) differs from what `cargo add solana-sdk` would pull today. This causes a dep conflict that eats 15+ minutes. Fix: add `use solana_sdk::compute_budget::ComputeBudgetInstruction;` to the snippet and note the version constraint or recommend deriving it from the workspace anchor version.

**[T2-MC-02] `sdk/rust/src/lib.rs:95` and `programs/alea-verifier/src/lib.rs:1` — `#[allow(unexpected_cfgs)]` is present with explanation in a comment, but the explanation is buried after the attribute and won't surface in rustdoc**

The attribute comment reads "Suppress Anchor 0.30.1's harmless `anchor-debug` cfg warning." That's correct and sufficient — internally. But `docs.rs` renders `lib.rs` module-level docs and an external consumer reading the crate doc page will see this attribute without the inline comment (rustdoc strips non-doc comments). A consumer doing a quick security scan (my standard practice before integrating any crypto crate) sees `#[allow(unexpected_cfgs)]` at module root with no rendered explanation and may walk away assuming the crate is suppressing something meaningful. Fix: add a `//! Note: ...` line in the module-level doc block explaining the allow, so it survives rustdoc rendering.

**[T2-MC-03] `sdk/rust/src/errors.rs:1-3` — `errors.rs` module doc links to `build-spec/sdk/rust-cpi.md` which is not accessible to external consumers**

The file says "See `build-spec/sdk/rust-cpi.md` for the full table." That path is inside the build-spec directory, which is not published to crates.io or docs.rs. A consumer following the link from rustdoc gets a dead reference. The full error code table is already in `sdk/rust/README.md`, which is published. Fix: change the link to point to the README section or reproduce the table inline in the rustdoc for `AleaError`.

**[T2-MC-04] `sdk/rust/src/lib.rs:163` — `is_round_recent` casts `clock.unix_timestamp` (i64) to u64 without negative guard; a malformed or pre-epoch timestamp silently wraps**

`let current_timestamp = clock.unix_timestamp as u64;` — if the sysvar somehow delivers a negative `unix_timestamp` (clock skew, localnet fixture misconfiguration, or an unusual Solana cluster state before the epoch start), the cast wraps to a very large u64. The `saturating_sub(round_timestamp)` then returns 0 (underflow saturates), so `0 <= max_age_seconds` is always true and stale-beacon protection silently passes. This is unlikely on production clusters but is a correctness trap for consumers testing on localnet with synthetic clocks. Fix: add a check `if clock.unix_timestamp < 0 { return false; }` before the cast, or at minimum document the assumption in the function doc.

---

### T3 — Nice to Have

**[T3-MC-01] `sdk/rust/Cargo.toml:6` — `rust-version = "1.79"` declared but no MSRV CI badge or CI job enforces it**

The MSRV is set but I can't verify from the public-facing files whether CI actually tests against 1.79. If `alea-verifier` or `anchor-lang` transitively requires a newer MSRV (which happens silently as deps update), the declared `rust-version` becomes false advertising. Consumer projects that pin older toolchains will get confusing errors. Recommend adding an MSRV CI job and noting in README that the MSRV is tested.

**[T3-MC-02] `sdk/rust/src/cpi.rs:47-61` — `verify` takes ownership of three `AccountInfo<'info>` values; the signature diverges from Anchor's own CPI pattern of passing references**

Anchor's generated CPI helpers take `AccountInfo` by value but document the pattern as "pass `.to_account_info()`" at the call site. The `alea_sdk::cpi::verify` wrapper does the same, which is idiomatic Anchor. However the rustdoc for the function doesn't explicitly call out that the caller must clone AccountInfos shared with other CPIs (e.g., `payer` is often also needed for a subsequent `token::transfer`). The README warns about return-data ordering but not about AccountInfo consumption. Minor, but a new Anchor dev will hit a borrow-checker error they won't immediately understand. A one-line note in the function doc would prevent this.

**[T3-MC-03] `programs/alea-verifier/src/errors.rs:38-43` — `InvalidFieldElement` (6003) is marked "currently unreachable" with no `#[deprecated]` or `doc(hidden)` annotation**

Consumer programs catching `AleaError::InvalidFieldElement` are writing dead match arms. The inline comment correctly notes this but a consumer reading generated rustdoc (which won't show the comment marker "currently unreachable") sees a live error code they may write handling logic for. Since it's reserved per ADR 0028 and not consumer-visible, adding `/// **Reserved. Currently unreachable.** Retained for CPI stability per ADR 0028.` to the variant doc (not just a `//` comment) would make this clear on docs.rs.

---

## Summary

| Tier | Count |
|------|-------|
| T1   | 2     |
| T2   | 4     |
| T3   | 3     |

**Top finding:** T1-MC-01 — the README quick-start snippet references `player_a`/`player_b` which don't exist in the snippet's scope. First code a new consumer tries to compile fails. This is a documentation error that costs zero trust but infinite first impressions.

**Second critical:** T1-MC-02 — the `cpi` feature flag is structurally broken (dep is unconditional). Any consumer who builds a crate topology based on the feature table will find the feature does nothing, and fixing it later would be a breaking change for any consumer who happened to be relying on the implicit unconditional behavior.

Security posture is notably strong for a v0.1: the mandatory-constraint warnings are unmissable, the return-data ordering footgun is documented in three places, and the example-lottery commit-reveal guard ordering has explicit load-bearing comments. Error codes are stable-pinned with tests. Would revisit after the two T1s are fixed.
