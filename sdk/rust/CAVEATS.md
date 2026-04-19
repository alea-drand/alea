# alea-sdk — Maturity Disclosures

This document is required reading before integrating alea-sdk into a production program. Six disclosures; each includes the phase where it is resolved.

---

## 1. Cluster Surface — Devnet Only

**Status:** Live on Solana devnet (`ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`). Validated across 10 live drand rounds. Mainnet deployment is the Phase 5 gate.

**Phase 5 resolution:** Mainnet deployment with identical program ID (vanity key usable on both clusters per ADR 0028). Mainnet traffic begins Phase 5 and CPI consumers on devnet migrate automatically by changing `--url`.

---

## 2. External Audit

**Status:** Internal audit clean — 10 Claude persona rounds + 5 Codex persona rounds, averaged 8.66/10 arbitrated score. All Tier-1 findings resolved or explicitly deferred to Phase 5. No external paid firm review has been performed.

**Phase 5 resolution:** Phase 5 gate requires a paid external audit before mainnet deployment. CPI interface is frozen per ADR 0028 — audit findings cannot require breaking changes to `verify` v1.

---

## 3. Upgrade Authority

**Status:** Program is currently controlled by the deployer keypair (single point of failure). Squads 2-of-3 multisig transition was committed in ADR 0009 but not yet executed (requires co-signers).

**Phase 5 resolution:** Multisig transition happens before mainnet deployment. Full timeline in ADR 0009. Immutable (authority zeroed) is planned post-mainnet-audit stabilization.

---

## 4. v1 CPI Interface — Not Yet Battle-Tested

**Status:** The v1 CPI interface (`verify(round, signature) -> [u8; 32]`) is frozen per ADR 0028 and validated across 4 audit rounds. The Pattern A auto-deserialize return path is proven via the `cpi-consumer` integration test (Phase 2 Wave 10). No breaking changes are planned — new capabilities ship as new instructions, never as modifications to `verify`.

However, the interface has not yet seen mainnet production traffic.

**Phase 5 resolution:** Mainnet production traffic and real consumer programs (Palestra, Phase 7) harden the interface empirically.

---

## 5. POST-T2.04 BPF 6006 None-Arm Runtime Test

**Status:** Open finding (convergent P10 + Codex audit finding). The `None` branch of the `verify_pairing` tri-state — which maps to `AleaError::PairingError` (6006) — can only be triggered by a real BPF syscall `Err` return (Agave / Firedancer infrastructure failure). This path has not been exercised in a live BPF environment.

The error code contract (6006) is stable and pinned by a native unit test (`pairing_error_6006_code_mapping_stable`). The branch is correct and audited; the gap is live BPF coverage of an infrastructure-failure path that is not easily induced in testing.

**Phase 5 resolution:** Phase 5 acceptance criteria includes a BPF-level test that injects a syscall error to exercise this branch.

---

## 6. Fuzzing Coverage

**Status:** 23.82 billion iterations across 3 parallel cargo-fuzz targets, 0 crashes, 0 memory errors. Proof tarballs published at GitHub release [`v0.2.0-audit-passed`](https://github.com/alea-drand/alea/releases/tag/v0.2.0-audit-passed).

Fuzzing is not a substitute for mainnet production hours. The targets cover field arithmetic, SVDW, and the pairing pipeline — not the full Anchor instruction surface.

**Phase 5 resolution:** Ongoing fuzz campaigns; extended coverage added with each audit round.
