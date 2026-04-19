# Persona: Ben Hughes — Rust Ecosystem Maintainer

**Role:** 10+ years Rust, 12 crates on crates.io (~800K combined downloads), docs.rs contributor. Reviewing `alea-sdk` + `alea-verifier` for crates.io publishability on behalf of a Foundation considering a grant.

**Scope read:** `sdk/rust/` (full tree), `programs/alea-verifier/Cargo.toml` + `src/`, root `Cargo.toml`, `README.md`, `CHANGELOG.md`, `LICENSE`.

**Mental model applied:** simulated `cargo package --list`, docs.rs render, semver story, dep hygiene, feature flag correctness, intra-doc link resolution, `[patch.crates-io]` downstream impact.

---

## Execution Notes

Checked all 10 mandate items in order. Ran mental `cargo package` against `sdk/rust/`. Traced every `pub use` re-export for intra-doc link resolution. Compared README error table against `errors.rs` enum declaration order and variant names. Checked `[patch.crates-io]` in root workspace. Verified `categories` against the canonical crates.io list. Simulated docs.rs feature activation path for `cpi` feature.

---

## Findings

### T1 — Publish-blocking or docs.rs-breaking

**[T1-1] `sdk/rust/README.md:36` — `AleaVerify` used in code example does not exist; actual export is `AleaVerifier`.**

The root `README.md` Quick Start Rust snippet (line 36) imports `alea_sdk::{self, AleaVerify}`. The actual re-exported type from `lib.rs:103` is `AleaVerifier` (sourced from `alea_verifier::program::AleaVerifier`). Any developer who copies this example gets an immediate compilation error. The `sdk/rust/README.md` and `lib.rs` module doc both use the correct `AleaVerifier` name — only the root README has the typo.

**[T1-2] `sdk/rust/README.md:179-184` — Error code table names do not match `AleaError` variant names in `errors.rs`; will mislead integrators matching on error codes.**

The README table lists `ChainHashMismatch` (6003), `InvalidChainHash` (6005), `InvalidPubkeyG2` (6007), `InvalidPublicKey` (6008). The actual `AleaError` variants in `programs/alea-verifier/src/errors.rs` are `InvalidFieldElement` (6003), `NoSquareRoot` (6004), `InvalidG2Point` (6005), `PairingError` (6006), `WrongChainHash` (6007), `WrongPubkey` (6008). The code-to-name mapping is wrong for every row from 6003 onward. Consumers pattern-matching on `AleaError::ChainHashMismatch` get a compile error; consumers reading the table to understand error 6005 get the wrong mental model. This is a docs-as-published correctness failure that will appear verbatim on docs.rs since the README is the crate README.

**[T1-3] `Cargo.toml (workspace):22-24` — `[patch.crates-io]` section exists (commented out) but is structurally present; uncommented patches poison all downstream consumers' lockfiles.**

The workspace `Cargo.toml` has an active `[patch.crates-io]` table with a commented-out `ark-ff` git override. The table header itself is present and parsed by Cargo even when all entries are commented out — this is harmless today, but the comment actively signals an intent to uncomment. More critically, any future uncomment of the ark-ff git patch would silently propagate to every consumer who resolves the workspace, breaking reproducibility for external users and causing crates.io publish to reject the tarball (patches are stripped from published crates but not from the workspace resolution that generates the tarball). This pattern needs a process guardrail: the patch table should be removed or relocated to a `[patch]` comment block outside the TOML table syntax.

**[T1-4] `sdk/rust/README.md:89` — `player_a` and `player_b` are undefined identifiers in the Quick Start code block (rendered as a non-`ignore` block on docs.rs).**

Line 89 in `sdk/rust/README.md` reads `let winner = if random_value % 2 == 0 { player_a } else { player_b };` inside a fenced Rust block that has no `ignore` annotation. docs.rs runs `cargo test --doc` on non-ignored code blocks; this block will fail to compile because `player_a` and `player_b` are not defined. The `lib.rs` module-doc equivalent uses `rust,ignore` correctly; the SDK README does not. Either add `rust,ignore` to the fence or define the variables as `let player_a = "Alice"; let player_b = "Bob";`.

---

### T2 — Should fix for quality

**[T2-1] `sdk/rust/Cargo.toml:13` — `categories = ["cryptography", "no-std"]` is misleading; this crate is NOT `no_std` (it imports `anchor-lang` which requires `std` on non-BPF targets).**

The `no-std` category on crates.io is reserved for crates that actually support `#![no_std]`. `alea-sdk` has no `#![no_std]` attribute, and `anchor-lang`'s dependency tree pulls in `std`. Listing `no-std` as a category will frustrate embedded developers who add the crate expecting it to work in a `no_std` context. Replace with `"cryptography::cryptocurrencies"` or `"wasm"` to reflect the Solana/BPF use case more accurately.

**[T2-2] `programs/alea-verifier/Cargo.toml:13-14` — `[package.metadata.docs.rs]` specifies `features = ["no-entrypoint"]` but docs.rs should build with `features = ["cpi"]` to render the public CPI API surface.**

The verifier's docs.rs config activates `no-entrypoint` but not `cpi`. The `cpi` feature is the gating flag that enables the CPI module consumers actually use. Without it, the `alea_verifier::cpi` module is absent from the docs.rs render, and the feature activation path `alea-sdk → alea-verifier/cpi` appears broken in the rendered output. Set `features = ["cpi"]` (which implies `no-entrypoint` via the feature dependency chain) for the verifier's docs.rs config.

**[T2-3] `CHANGELOG.md:14-15` — `[0.1.0]` entry is dated `TBD` and contains a hackathon deadline note in the changelog body; changelog should not embed project timeline commentary.**

The CHANGELOG entry reads `## [0.1.0] — TBD (target: before May 11, 2026 — Colosseum Frontier submission)`. Keep-a-Changelog format expects either an ISO date (`2026-05-11`) or `[Unreleased]`. The hackathon target embedded in the version header will be permanently visible in the release history. Move the date to a real value before publishing, and strip the submission deadline. The `[Unreleased]` section also contains pre-v0.1.0 spec notes that belong in commit history, not a published changelog.

**[T2-4] `sdk/rust/Cargo.toml:43` — `rustdoc-args = ["--cfg", "docsrs"]` is specified but no code in `sdk/rust/src/` uses `#[cfg(docsrs)]` or `#[cfg_attr(docsrs, ...)]`; the flag is a no-op and may confuse future maintainers.**

The `docsrs` cfg flag is conventionally used to gate `#[doc(cfg(...))]` attribute annotations on feature-gated items. Since no such annotations exist in the SDK source, the flag does nothing. Either add `#[doc(cfg(feature = "cpi"))]` on the `cpi` module to make it useful, or remove the `rustdoc-args` line to avoid confusion.

**[T2-5] `sdk/rust/Cargo.toml` — no `authors` field; crates.io will show an empty author list.**

This is not a publish blocker (authors is optional since Cargo 1.63) but is expected for ecosystem-facing crates. The LICENSE file names `Aaron Kruger / alea-drand contributors` — the same attribution should appear in the manifest.

**[T2-6] `sdk/rust/Cargo.toml` — no `exclude` field; `cargo package --list` will bundle the `tests/` directory but also any workspace-level artifacts that happen to be reachable via symlink.**

Without an explicit `exclude` or `include`, Cargo packages everything reachable from the crate root. The `tests/` directory (including `devnet_clock.rs` with pinned Solana RPC URLs) will be included in the published tarball — this is fine, but `devnet_verify.rs` and `fixtures.rs` should be reviewed to ensure they do not embed private keys or privileged RPC endpoints. A minimal `exclude = ["tests/devnet_*"]` or explicit `include` list communicates intentionality.

---

### T3 — Polish

**[T3-1] `sdk/rust/Cargo.toml:12` — `keywords` are strong but `"bls"` is low-signal; consider replacing with `"vrf"` or `"beacon"` to match search terms developers actually use.**

`["solana", "drand", "randomness", "bls", "bn254"]` — at the five-keyword limit (good). `"bls"` is accurate but `"vrf"` has 3x more crate-search traffic and better describes what end-consumers are looking for (verifiable randomness). `"beacon"` would capture drand-aware consumers. Minor discoverability gain if one keyword is swapped.

**[T3-2] `sdk/rust/Cargo.toml:8` — `homepage = "https://alea.so"` — verify the domain resolves before publishing; a 404 homepage is flagged by the crates.io quality linter.**

Docs say the docs site is "Coming Phase 6." If `alea.so` does not resolve at publish time, crates.io will show a broken homepage link. Use the GitHub repository URL as homepage fallback until the site is live, then update in a patch release.

**[T3-3] `programs/alea-verifier/src/lib.rs:19` — `pub use instructions::map_to_point_debug::*` re-exports a debug instruction unconditionally; it should be gated behind a feature or at minimum documented in the crate-level rustdoc.**

The `map_to_point_debug` instruction is always compiled into the binary and always re-exported. The inline comment explains it is "stateless pure function with zero attack surface." That explanation belongs in the public rustdoc (a `///` doc comment on the `pub use` or the handler), not just in an inline comment visible only in source. docs.rs will render this re-export without any documentation string, which looks like an oversight.

---

## Summary

Three T1 issues require fixes before `cargo publish`: a wrong type name (`AleaVerify` vs `AleaVerifier`) in the root README, an error code table in the SDK README that maps every code from 6003 onward to the wrong variant name, and undefined identifiers (`player_a`, `player_b`) in a non-ignored doc-test block that will fail `cargo test --doc`. The workspace `[patch.crates-io]` table is structurally present and commented — it should be removed before any partner attempts to take a workspace dependency. Four T2 issues are quality concerns: the `no-std` category is false advertising, the verifier's docs.rs feature config misses `cpi`, the CHANGELOG embeds a hackathon deadline, and the `docsrs` rustdoc-arg is a no-op. T1-1 and T1-2 are the fastest to fix (two README edits); T1-4 is one fence annotation.
