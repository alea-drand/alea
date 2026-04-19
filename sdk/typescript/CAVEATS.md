# @alea-drand/sdk — Maturity Disclosures

Five things to know before using this SDK in production.

---

## 1. Cluster Surface — Devnet Live, Mainnet Status

The program is live on **Solana devnet** at `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`, verified end-to-end against live drand rounds.

Alea's program ID is cluster-agnostic — the SDK exports both `DEVNET_PROGRAM_ID` and `MAINNET_PROGRAM_ID` pointing to the same bytes (distinct symbols for intent clarity). Your `Connection` object selects the cluster. If mainnet isn't yet live when you read this, a mainnet `Connection` fails at the Solana RPC layer with "program not found" — Solana itself is the safety rail. Check the main [README.md](../../README.md) for current deployment status.

**What would close this:** mainnet deployment.

---

## 2. External Audit

Alea has undergone self-imposed multi-pass adversarial review prior to publication, plus a combined **23.82 billion iterations** of fuzz testing across 5 cargo-fuzz targets with 0 crashes, 0 memory errors. Proof tarballs + coverage HTML + per-target metadata are published at the [`v0.2.0-audit-passed`](https://github.com/alea-drand/alea/releases/tag/v0.2.0-audit-passed) GitHub release.

No external paid security audit has been performed. The internal testing is extensive, but it's not the same thing as a firm-signed audit report.

**What would close this:** an external paid audit. This is a goal if/when grant funding supports it — not a v0.1.0 commitment.

---

## 3. Upgrade Authority — Single Deployer Key

The verifier program is currently upgradeable, controlled by a single deployer keypair. Migration to a Squads 2-of-3 multisig is planned after mainnet stabilises, and the long-term intent is to zero out the upgrade authority entirely. Alea holds no user funds on-chain — there's no TVL surface for an authority compromise to attack — but a compromised key could replace the deployed program binary. Consumers wanting belt-and-suspenders can verify the deployed binary SHA256 against the value published in the main README.

**What would close this:** multisig transition, then immutability.

---

## 4. v1 CPI Interface — Frozen, Not Yet Battle-Tested at Scale

The v1 CPI interface (`verify(round, signature) -> [u8; 32]`) is frozen: instruction signature, Config layout, Verify accounts, return-data format, and `BeaconVerified` event schema are not modified by upgrades. Additive changes (new instructions) can happen at minor versions; breaking changes require a new program ID. CI enforces this on every PR.

The interface hasn't yet seen high-volume production traffic, so edge cases specific to real-world consumer patterns are unknown.

**What would close this:** real-world mainnet usage across a variety of consumer programs.

---

## 5. BPF Runtime Error Path — One Open Item

A specific runtime error path (`AleaError::PairingError`, code 6006) can only be triggered by a real BPF syscall `Err` return. This has not yet been exercised in a live BPF environment. The error code contract is stable; the branch is correct and audited. The gap is live BPF coverage of a path that's not easy to induce.

**What would close this:** a BPF-level test that injects a syscall error to exercise this branch.
