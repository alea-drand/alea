# @alea-drand/sdk — Maturity Disclosures

Six things to know before using this SDK in production.

---

## 1. Devnet Only (Phase 5 Resolution: Mainnet Deploy)

The program at `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U` is deployed on **Solana devnet only**. `MAINNET_PROGRAM_ID` throws intentionally — there is no mainnet program yet. Phase 5 deploys to mainnet and publishes a new SDK release with the live ID.

## 2. Internal Audit Only (Phase 5 Resolution: External Paid Audit)

The Alea verifier has passed multi-pass internal audits: 15-round persona audit (10 Claude + 5 Codex, averaged 8.66/10, zero critical findings), plus a pre-publish 12-agent audit (8 cold-read personas + 4 adversarial red-team agents) run 2026-04-19 — zero exploitable cryptographic or replay vulnerabilities, zero T1 findings against mandatory-constraint-following consumers. An external paid security firm audit is planned for Phase 5 before mainnet. Do not use in production until that audit is complete.

## 3. Deployer Keypair (Phase 5 Resolution: Squads 2-of-3 Multisig)

Upgrade authority is currently held by a single deployer keypair. A Squads 2-of-3 multisig transition is planned per ADR 0009. Until then, the deployer key is a single point of failure for program upgrades. Phase 5 completes the multisig transition.

## 4. CPI Interface v1 Frozen (Phase 5 Resolution: Battle-Tested Mainnet)

The v1 CPI interface is frozen per ADR 0028, validated across 4 audit rounds and a cpi-consumer Pattern A integration test. It has not yet been exercised in a mainnet environment. Additive changes may occur in v2+; breaking changes will follow semver with a deprecation period.

## 5. BPF 6006 None-Arm Runtime Test Open (Phase 5 Resolution: Confirmed)

An open item exists for the `PairingError` (6006) code path: the BPF `alt_bn128_pairing` syscall's `None`-arm (failure case) has not been exercised in a live runtime test. This is a convergent finding from internal + Codex audit rounds (P10). The happy path is fully tested across 10+ devnet rounds. Phase 5 closes this gap with an explicit failure-injection test.

## 6. Fuzz Coverage (Not a Production-Hours Substitute)

23.82 billion fuzzing iterations have been run against the Alea verifier with 0 crashes found. Proof tarballs are published at [alea-drand/alea](https://github.com/alea-drand/alea). This is a strong signal, not a guarantee. Fuzzing complements but does not replace production runtime hours or a paid security audit.
