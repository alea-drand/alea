# alea-sdk — Maturity Disclosures

This document is required reading before integrating alea-sdk into a production program. Five disclosures; each includes what would close it.

---

## 1. Cluster Surface — Devnet Live, Mainnet Status

The program is live on **Solana devnet** at `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`, verified end-to-end against live drand rounds.

Mainnet deploys with the same vanity program ID (the program ID is cluster-agnostic). If mainnet isn't yet live when you read this, a mainnet `Connection` will fail at the Solana RPC layer with "program not found"; check the main [README.md](../../README.md) for current deployment status.

**What would close this:** mainnet deployment and production usage.

---

## 2. External Audit

Alea has undergone self-imposed multi-pass adversarial review prior to publication, plus a combined **23.82 billion iterations** of fuzz testing across 5 cargo-fuzz targets with 0 crashes or memory errors. Proof tarballs + coverage HTML + per-target metadata are attached to the [`v0.2.0-audit-passed`](https://github.com/alea-drand/alea/releases/tag/v0.2.0-audit-passed) GitHub release.

No external paid security audit has been performed yet. The internal testing is extensive, but it's not the same thing as a firm-signed audit report.

**What would close this:** an external paid audit. This is a goal if/when grant funding supports it — not a v0.1.0 commitment.

---

## 3. Upgrade Authority

The program is currently upgradeable, controlled by a single deployer keypair. This is a single point of failure: a compromise of that key would let an attacker replace the deployed program binary.

Mitigations:
- Planned migration to a Squads 2-of-3 multisig after mainnet stabilises.
- Long-term intent is to zero out the upgrade authority entirely.
- Consumers wanting belt-and-suspenders can pin to the binary SHA256 (published in the README's Program Addresses section) and refuse to interact if the deployed binary changes unexpectedly.
- Alea holds no user funds on-chain — there's no TVL surface for an authority compromise to drain.

**What would close this:** multisig transition, then immutability.

---

## 4. v1 CPI Interface — Frozen But Not Yet Battle-Tested at Scale

The v1 CPI interface (`verify(round, signature) -> [u8; 32]`) is intentionally frozen: the instruction signature, `Config` account layout, `Verify` accounts struct, return-data format, and `BeaconVerified` event schema are not modified by upgrades. New capabilities ship as new instructions at minor versions; breaking changes would require a new program ID (a new deployment, not an upgrade). CI enforces this via the `idl-diff` check on every PR.

The interface hasn't yet seen high-volume production traffic, so edge cases specific to real-world consumer patterns are unknown.

**What would close this:** real-world mainnet usage across a variety of consumer programs.

---

## 5. BPF Runtime Error Path — One Open Item

A specific runtime error path in the `alt_bn128_pairing` syscall (mapping to `AleaError::PairingError`, code 6006) can only be triggered by a real BPF syscall `Err` return — an infrastructure failure path. This has not yet been exercised in a live BPF environment. The error code contract is stable and pinned by a native unit test; the branch is correct and audited, but the gap is live BPF coverage of a path that's not easy to induce in testing.

**What would close this:** a BPF-level test that injects a syscall error to exercise this branch.
