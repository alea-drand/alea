# Contributing to Alea

Thanks for your interest in contributing. Alea is a small, public-good project — contributions are welcome and reviewed personally by Aaron (sole maintainer; see CHANGELOG.md and the public docs at https://alea.so for the multisig transition roadmap + tiered maintenance commitment).

## Solo-Maintainer Caveat

Alea is currently **Tier A** (default, no grant): 3 months active support post-mainnet. Response times for issues and PRs are **best-effort** until Tier B activates via grant funding. If you need guaranteed response times for a commercial integration, surface it early via a GitHub issue — we'll figure out a path (e.g., Tier B support if a grant is in-flight, or a fork-and-maintain recommendation).

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | **TBD — pinned in `rust-toolchain.toml` after Phase 1.1.A** | [rustup.rs](https://rustup.rs/) |
| Solana CLI | 2.2.x | [docs.solanalabs.com](https://docs.solanalabs.com/cli/install) |
| Anchor | **`=0.30.1`** (exact pin — see public docs at https://alea.so for rationale) | `cargo install --git https://github.com/coral-xyz/anchor avm --locked --force && avm install 0.30.1 && avm use 0.30.1` |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org/) |
| pnpm (recommended) | ≥ 8 | `npm install -g pnpm` |

Confirm installation:
```bash
rustc --version                          # matches rust-toolchain.toml
solana --version                         # 2.2.x
anchor --version                         # 0.30.1
node --version                           # ≥ 18
```

## Clone and Build

```bash
git clone https://github.com/alea-drand/alea.git
cd alea
anchor build                             # builds the program + IDL
pnpm install                             # or npm install — installs TS SDK workspace deps
```

## Running Tests

```bash
# Unit tests (Rust, native target — fast)
cargo test --workspace

# On-chain tests (Anchor localnet)
anchor test

# TypeScript SDK tests
cd sdk/typescript && pnpm test

# Regenerate test vectors (only when spec changes require it — see ADR 0029)
cd testing/scripts   # T3.v — was build-spec/testing/scripts; build-spec is private, scripts live at testing/scripts in the public repo (mirrored at publish time)
pnpm install
node --experimental-strip-types generate-test-vectors.ts
# Commit updated fixtures if they changed; investigate if the change is
# unexpected (drand API drift? scheme change?)
```

All tests MUST pass before opening a PR.

## Pull Request Expectations

1. **Description required** — explain what and why, not just what.
2. **CI must pass** — green tests, clippy, and lint.
3. **ADR 0028 compliance** — no breaking changes to `verify` v1 (signature, account layout, return data, event schema). Additive changes only. The PR template has a checklist.
4. **ADR 0034 compliance** — any CPI example code must include `seeds::program = alea_program.key()` on the Alea config account. Examples without this constraint ship exploitable programs; a missing constraint blocks merge.
5. **Test coverage** — new behavior gets tests. Regressions get regression tests. Don't break existing test vectors (`testing/fixtures/*.json`) without regenerating + committing + explaining in PR description.
6. **Spec alignment** — if the change touches program semantics, the spec source-of-truth is the public docs site at https://alea.so (Phase 6 deliverable). Until the docs site ships, the maintainer holds the canonical spec privately; PR reviewers cite the relevant public-docs section in review feedback.

## Versioning Policy (ADR 0028)

Alea follows semver with Alea-specific constraints:

- **patch** (`0.1.0 → 0.1.1`): bug fixes only. No interface changes. Safe auto-update for consumers.
- **minor** (`0.1.x → 0.2.0`): new instructions (additive). Existing instructions unchanged. Safe for consumers who don't use the new instruction.
- **major** (`0.x.y → 1.0.0`): reserved for "graduation from initial release to stable" (post-audit + 6+ months mainnet without critical bugs). Still preserves v1 `verify` semantics.
- **Breaking changes to `verify` v1**: forbidden. Would require a new mainnet program ID (new deployment, not an upgrade). If you believe a breaking change is needed, open an RFC issue — do NOT send a PR.

The `verify` signature, `Config` layout, return data format, account list, and `BeaconVerified` event are frozen forever at the mainnet program ID. New capabilities ship as new instructions (`verify_recent`, `verify_batch`, etc.).

## Code Style

- **Rust:** `cargo fmt` + `cargo clippy --workspace -- -D warnings` before commit. CI enforces.
- **TypeScript:** Prettier + strict TS (`strict: true`). No `any` without justification comment.
- **Comments:** Required for non-obvious crypto/consensus logic. Prefer explaining *why* (the invariant) over *what* (code does).
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

**Do NOT open public GitHub issues for security vulnerabilities.** See [`.github/SECURITY.md`](.github/SECURITY.md) for the private disclosure process.

## Reference Material

- [`README.md`](README.md) — project overview, integration example, security pointers
- [`NOTICE`](NOTICE) — third-party attributions (drand, kevincharm/bls-bn254, randa-mu/bls-solana, noble libraries)
- [`CHANGELOG.md`](CHANGELOG.md) — versioning policy + release history
- Public docs site: https://alea.so — full spec, ADRs, threat model (Phase 6 deliverable)
- [`.github/SECURITY.md`](.github/SECURITY.md) — disclosure process

T3.v note: `build-spec/` is PRIVATE per the visibility model and is not shipped in the public repo. Cite public-repo references (above) in reviews and contributor docs; cite `https://alea.so/...` URLs once the docs site lands at Phase 6.

## Code of Conduct

Be respectful. Disagreements are fine; contempt is not. If you're unsure whether a comment crosses a line, err on the side of kindness.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0 (same as the project). See [LICENSE](LICENSE) for full text.
