# Security Policy

## Reporting a Vulnerability

**Primary:** [Open a private security advisory](https://github.com/alea-drand/alea/security/advisories/new). GitHub notifies maintainers immediately and provides a private discussion channel.

**Fallback:** `security@alea.so` — served by a self-hosted mail server; inbound-only with a reply from the maintainer's personal mailbox. Until the mail server is provisioned, the GitHub Security Advisory route above is canonical.

Include: a description of the vulnerability, reproduction steps, impact assessment, and any proof-of-concept code (keep local; do not deploy to mainnet).

## Response Intent

Alea is solo-maintained and grant-unfunded. The governance roadmap (see [README.md §Governance](https://github.com/alea-drand/alea#governance--upgrade-roadmap)) describes the planned transition to a Squads 2-of-3 multisig within 90 days of mainnet deploy (or at the first external paid audit — whichever fires first).

**Pre-transition:** best-effort response within a few days for P0 issues. The maintainer reads security advisories daily. If the maintainer is unreachable for a prolonged period, the Apache 2.0 license permits forks under a new program ID; fork migration instructions would be published in CHANGELOG.md and any open security advisory.

**Post-transition:** target SLAs expand to 72h acknowledgement / 7d triage / 30d P0 fix, with multi-signer triage fallback.

## Scope

**In scope:**
- `alea-verifier` on-chain program (`programs/alea-verifier/`)
- `@alea-drand/sdk` TypeScript SDK (`sdk/typescript/`)
- `alea-sdk` Rust CPI crate (`sdk/rust/`)
- Specification flaws that cascade into implementation bugs

**Out of scope:**
- Consumer program economic exploits (game logic, market design) — these are the integrating application's responsibility
- drand network-level issues (DKG, threshold signatures) — report to [drand](https://github.com/drand/drand/security/advisories)
- Solana runtime bugs (alt_bn128 syscalls, validator) — report to [Solana](https://github.com/solana-labs/solana/security/advisories) or [Anza](https://github.com/anza-xyz/agave)
- BN254 curve-level attacks — Alea inherits the ~100-bit security level of BN254
- Denial-of-service via CU exhaustion on malformed signatures — expected behaviour, not a security bug

## Disclosure Timeline

Coordinated disclosure. After a report: private discussion → fix development → public advisory (with CVE if applicable) → patch release via `.github/workflows/release.yml` → CHANGELOG.md entry with credit.

## Bug Bounty

Intent documented; activation post-grant. Until then, reporters are credited publicly (with permission) in release notes. For high-severity issues, non-monetary recognition or introductions may be offered at the maintainer's discretion.

## References

- [README.md §Governance](https://github.com/alea-drand/alea#governance--upgrade-roadmap) — upgrade authority roadmap
- [`audit/phase-4.5/THREAT-MODEL.md`](../audit/phase-4.5/THREAT-MODEL.md) — trusted-vs-untrusted surface enumeration
- [`audit/phase-4.5/FINDINGS-CONSOLIDATED.md`](../audit/phase-4.5/FINDINGS-CONSOLIDATED.md) — Phase 4.5 audit findings
