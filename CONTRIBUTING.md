# Contributing to Alea

Thanks for your interest in contributing. Alea is a small, public-good project — contributions are welcome and reviewed personally by the sole maintainer. See the main [README.md](README.md) §Governance + §Maintenance Tiers and [CHANGELOG.md](CHANGELOG.md) for the multisig transition roadmap and tier commitments.

## Solo-Maintainer Caveat

Alea is currently solo-maintained and grant-unfunded. Response times for issues and PRs are best-effort. If you need guaranteed response times for a commercial integration, surface it early via a GitHub issue — we'll figure out a path (prioritised support if grant funding is in flight, or a fork-and-maintain recommendation).

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | Stable 1.79+ (see `rust-toolchain.toml`) | [rustup.rs](https://rustup.rs/) |
| Solana CLI | 2.2.x | [docs.solanalabs.com](https://docs.solanalabs.com/cli/install) |
| Anchor | `=0.30.1` (exact pin — ADR 0028 CPI stability) | `cargo install --git https://github.com/coral-xyz/anchor avm --locked --force && avm install 0.30.1 && avm use 0.30.1` |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org/) |

Confirm installation:
```bash
rustc --version                          # 1.79 or newer
solana --version                         # 2.2.x
anchor --version                         # 0.30.1
node --version                           # ≥ 18
```

## Clone and Build

```bash
git clone https://github.com/alea-drand/alea.git
cd alea

# Rust workspace (all crates)
cargo check --workspace --all-targets

# TypeScript SDK
cd sdk/typescript && npm ci && npm run build
```

**External consumer note:** if `cargo build-sbf` fails with `constant_time_eq@0.4.3 requires rustc 1.95`, pin `constant_time_eq = "=0.4.2"` in your consumer's `Cargo.toml` — Solana BPF rustc lags the ecosystem by ~6 minor versions. See [`sdk/rust/README.md`](sdk/rust/README.md) §Troubleshooting.

## Running Tests

```bash
# Unit tests (Rust, native target — fast)
cargo test --workspace --lib --tests

# TypeScript SDK tests
cd sdk/typescript && npm test

# Live devnet integration tests (gated by --ignored / env var)
cargo test -p alea-sdk --test devnet_verify -- --ignored
cd sdk/typescript && ALEA_DEVNET_TESTS=1 npm test
```

All PRs must pass the CI suite (see `.github/workflows/`): `lint`, `test`, `idl-diff`, `sdk-ts`, `supply-chain` (cargo-deny + npm audit + trufflehog).

## Pull Request Expectations

1. **Description required** — explain what and why, not just what.
2. **CI must pass** — green tests, clippy, lint, supply-chain scan.
3. **ADR 0028 compliance** — no breaking changes to `verify` v1 (signature, account layout, return data, event schema). Additive changes only. The PR template has a checklist. Breaking changes to the on-chain contract are enforced by the `idl-diff` CI job.
4. **Mandatory-constraint compliance** — any CPI example code must include `seeds::program = alea_program.key()` on the Alea config account and demonstrate `is_round_recent()` before trusting randomness. Examples without these constraints ship exploitable programs and block merge.
5. **Test coverage** — new behavior gets tests. Regressions get regression tests. Don't break existing test vectors without an explicit commit explaining why.
6. **Conventional commits** — see Commit Style below.

## Versioning Policy

Alea follows semver with project-specific constraints:

- **patch** (`0.1.0 → 0.1.1`): bug fixes only. No interface changes. Safe auto-update for consumers.
- **minor** (`0.1.x → 0.2.0`): new instructions (additive). Existing instructions unchanged.
- **major** (`0.x.y → 1.0.0`): reserved for "graduation from initial release to stable" (post-audit + 6+ months mainnet without critical bugs). Still preserves v1 `verify` semantics.
- **Breaking changes to `verify` v1**: forbidden. Would require a new mainnet program ID (new deployment, not an upgrade). If you believe a breaking change is needed, open an RFC issue — do NOT send a PR.

The `verify` signature, `Config` layout, return-data format, accounts struct, and `BeaconVerified` event are **frozen forever** at the mainnet program ID. New capabilities ship as new instructions (`verify_recent`, `verify_batch`, etc.).

## Code Style

- **Rust:** `cargo fmt --all` + `cargo clippy --all-targets -- -D warnings` before commit. CI enforces.
- **TypeScript:** Prettier + strict TS (`strict: true`). No `any` without justification comment.
- **Comments:** required for non-obvious crypto/consensus logic. Prefer explaining *why* (the invariant) over *what* (code does).
- **No `unsafe` Rust** except with justification comment citing an ADR or a specific Solana/ark-ff API constraint.

## Commit Style

Conventional commits strongly preferred:
- `feat: add verify_recent instruction` (new capability)
- `fix: correct sgn0 Montgomery LSB bug` (bug fix)
- `docs: update CPI integration example` (documentation)
- `refactor: extract g1_negate helper` (no behavior change)
- `test: add round 9337227 regression test`
- `chore: bump anchor to 0.30.2` (dependency)

Commits that span multiple areas get separate commits. A PR can have multiple commits — prefer clarity over minimal commit count.

## Security Issues

**Do NOT open public GitHub issues for security vulnerabilities.** See [`.github/SECURITY.md`](.github/SECURITY.md) for the private disclosure process (GitHub Security Advisory + email fallback).

## Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — we follow the Contributor Covenant v2.1.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0 (same as the project). See [LICENSE](LICENSE) for full text.

## Reference Material

- [`README.md`](README.md) — project overview, integration example, API reference, error codes, audit trail, governance
- [`CHANGELOG.md`](CHANGELOG.md) — versioning policy + release history
- [`NOTICE`](NOTICE) — third-party attributions (drand, kevincharm/bls-bn254, randa-mu/bls-solana, noble libraries)
- [`sdk/rust/README.md`](sdk/rust/README.md) — Rust CPI integration guide + troubleshooting
- [`sdk/typescript/README.md`](sdk/typescript/README.md) — TypeScript SDK reference
- [`audit/phase-4.5/`](audit/phase-4.5/) — Phase 4.5 audit findings + threat model
- [`validation-report.md`](validation-report.md) — validation evidence across phases
- [`.github/SECURITY.md`](.github/SECURITY.md) — vulnerability disclosure process
