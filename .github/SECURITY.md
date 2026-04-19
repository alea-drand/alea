# Security Policy

## Reporting a Vulnerability

**Primary:** [Open a private security advisory](https://github.com/alea-drand/alea/security/advisories/new). GitHub notifies maintainers immediately and provides a private discussion channel.

**Fallback:** `security@alea.so` (served by self-hosted Stalwart Mail on Alea's VPS, inbound-only; reply comes from maintainer's personal mailbox). Until the mail server is provisioned, the GitHub advisory is the canonical route.

Include: a description of the vulnerability, reproduction steps, impact assessment, and any proof-of-concept code (keep local; do not deploy to mainnet).

## Response Intent

Alea is solo-maintained until the Squads 2-of-3 multisig transition (trigger: 90 days post-mainnet, $50K TVL, or first external audit — whichever fires first). See [README.md §Governance](https://github.com/alea-drand/alea#governance--upgrade-roadmap) for the full roadmap.

**Pre-transition:** best-effort response within a few days for P0 issues. If the maintainer is unreachable, the Apache 2.0 license permits forks under a new program ID; fork migration instructions would be published in CHANGELOG.md and the upstream security advisory.

**Post-transition:** response SLAs expand to 72h acknowledgment / 7d triage / 30d P0 fix, with multi-signer triage fallback. See ADR 0035 for the full commitment.

## Scope

**In scope:**
- `alea-verifier` on-chain program (`programs/alea-verifier/`)
- `@alea/sdk` TypeScript SDK (`sdk/typescript/`)
- `alea-sdk` Rust CPI crate (`sdk/rust/`)
- Specification flaws that cascade into implementation bugs

**Out of scope:**
- Consumer program economic exploits (game logic, market design) — see `architecture/security-model.md` Threat 6
- drand network-level issues (DKG, threshold signatures) — report to [drand](https://github.com/drand/drand/security/advisories)
- Solana runtime bugs (alt_bn128 syscalls, validator) — report to [Solana](https://github.com/solana-labs/solana/security/advisories)
- BN254 curve-level attacks — Alea inherits the ~100-bit security level
- Denial-of-service via CU exhaustion on malformed signatures — expected behavior, not a security bug

## Disclosure Timeline

Coordinated disclosure. After a report: private discussion + fix development → public advisory with CVE if applicable → patch release via `.github/workflows/release.yml` → CHANGELOG.md entry with credit.

## Bug Bounty

Intent documented; activation post-grant. Until then, reporters are credited publicly (with permission) in release notes. For high-severity issues, non-monetary recognition or introductions may be offered at the maintainer's discretion.

## References

- ADR 0009 — Upgrade authority roadmap + trust-breaking commitment
- ADR 0035 — Full security disclosure policy
- `architecture/security-model.md` — 7-threat model including fake-config substitution (ADR 0034)
