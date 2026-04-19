# Alea — Threat Model (Phase 4.5)

Explicit trust-boundary document. One page. Reviewed Phase 4.5 (2026-04-19) by 8 persona + 4 red-team agents; re-review scheduled for Phase 5 external audit.

## Trusted

| Surface | Why trusted | Failure mode if trust breaks |
|---|---|---|
| Solana runtime (syscalls, BPF VM) | Deployed chain code we execute under | All Solana programs fail; Alea cannot be separately hardened against |
| `alt_bn128_pairing` syscall | Solana-provided, deterministic | A runtime bug silently flipping the pairing result would let forged signatures verify — depends on validator consensus to catch |
| ark-ff / ark-bn254 / ark-ec (0.5.0 pinned via workspace) | Audited arkworks cryptography; used with fork-tested constants | A subtle constant miscompile would produce silently-wrong curve points |
| drand League of Entropy | Threshold-signed by 23 independent organizations (Cloudflare, Protocol Labs, EF, etc.); on-chain pairing check rejects any impostor | Compromise of t+1 drand members could forge signatures — mitigated by the threshold scheme |
| Aaron's deployer keypair (pre-Phase-5) | Controls upgrade authority; stored in 1Password with 2FA | Key compromise → attacker can deploy malicious binary. Phase 5 multisig transition closes this. Users can lock to a specific binary hash for protection in the interim. |
| Aaron's npm + crates.io publish tokens | Controls SDK package releases | Token leak → malicious version published. Rotate immediately; yank/deprecate affected versions; users on pinned versions unaffected |

## Untrusted

| Surface | Mitigation |
|---|---|
| Consumer user input (`round`, `signature` bytes) | SDK validates length + hex at boundary (Phase 4.5 T1-03/04/06); on-chain verify rejects invalid G1 points (6001), wrong signatures (6000), round=0 (6002) |
| drand API transport | TLS only; 5-endpoint fallback; response size cap (4KB); `redirect: "error"` blocks CDN-compromise redirects; round-mismatch detection (Phase 4.5 T1-02) |
| Any single drand endpoint | 5-endpoint fallback with per-request timeout + overall retry cap. Worst case: all 5 compromised → SDK throws `DrandFetchFailed` (6100) or on-chain pairing fails (6000) — never silently returns attacker randomness |
| Consumer program | SDK enforces nothing on-chain for the consumer's state. Mandatory `seeds::program` + `is_round_recent` are consumer-layer responsibilities, documented in README + enforced by copy-paste pattern in example-lottery. Phase 4.5 added `cpi::verify` runtime owner check as defense-in-depth (T1-08) |
| Solana RPC endpoint (consumer-supplied) | Can censor or forge tx results; consumer picks their RPC. SDK reads `meta.err` from `getTransaction` which is RPC-attested. Users should use trusted RPC (own validator or reputable provider) |
| Consumer's wallet adapter | SDK validates `signTransaction` is a function before calling (T2-16); hardware/watch wallets rejected cleanly; no seed material touched |

## Attack Vectors In-Scope

1. **Fake Config PDA substitution** — attacker passes a program-owned Config PDA with attacker-chosen G2 pubkey. `seeds::program = alea_program.key()` on consumer side re-derives PDA under Alea's ID; Anchor rejects. Phase 4.5 added runtime owner check as second line (T1-08).
2. **Replay of old drand round** — `is_round_recent` rejects beacons older than consumer's threshold (e.g., 30s). Phase 4.5 future-round accept was aligned between Rust + TS (intentional tradeoff; narrows replay window but avoids sub-second-skew false-rejects).
3. **Commit-reveal front-running** — consumer enforces `min_resolution_round ≥ current_round + 1` (example-lottery pattern). Player cannot resolve on a round they observed at commit time.
4. **Compromised drand endpoint** — returns valid-but-wrong-round signature. Phase 4.5 T1-02: fetchBeacon verifies returned `data.round` matches requested `targetRound` before using the signature.
5. **Return-data corruption via re-entrant CPI** — consumers must capture `cpi::verify` return data before any other CPI in the same tx. Documented + `#[must_use]` attribute flags ignored returns at compile time.
6. **Error-code information leak** — no error code carries keypair material, wallet pubkeys in sensitive form, or internal state. Reviewed.
7. **Denial-of-service via slow drand endpoint** — 3 retries × 5 endpoints × 5s timeout = 77s worst-case hang. Phase 4.5 added AbortSignal threading (T2-15) so consumers can cancel mid-loop.
8. **Consumer CPI consumer's payer being spoofed** — `payer` is marked `Signer<'info>` on Alea's side; multi-signer tx attribution is a consumer concern (`BeaconVerified.payer` records whichever signer the consumer routed).
9. **CU exhaustion** — consumer must include `ComputeBudgetInstruction::set_compute_unit_limit(900_000)`. TS SDK injects automatically; Rust README §Troubleshooting covers the manual case.
10. **Malformed 64-byte G1 input** — ~2⁻²⁵⁶ per random attempt produces an on-curve point; even so, wrong for any specific round → pairing fails → 6000.

## Out-of-Scope

- **BN254 curve-level attacks** (small-subgroup on G1 — cofactor is 1, so non-issue; discrete log hardness assumption)
- **Solana runtime correctness** (consensus, BPF VM bugs — depend on validator software quality)
- **ark-ff / arkworks library correctness** (audited cryptography; we trust the library but cross-validate constants against Solidity reference + gnark-crypto at Phase 1)
- **Consumer game-logic bugs** (house-edge accounting, payout math, bet sizing) — the SDK's job is to supply correct randomness, not to validate the game
- **Drand network consensus failures** (fewer than t+1 honest parties — mitigated by League of Entropy's diverse membership)
- **Physical key theft** (Aaron's deployer keypair on M5 Max + 1Password) — covered by the Phase 5 multisig transition

## Review History

| Date | Scope | Findings |
|---|---|---|
| 2026-04-14 | 12-persona internal audit (spec only) | 104 findings, 22 T1 resolved |
| 2026-04-16 | R2 audit + fix pass | 18 T1 resolved, 6 ADRs updated |
| 2026-04-17 | Fuzz + cross-model (Codex) audit | 23.82B iter, 0 crashes; FENDER-002 + BPF 6006 finding |
| 2026-04-19 | **Phase 4.5 pre-publish** (this audit) | 16 T1 + ~28 T2 + ~25 T3. Zero exploitable crypto/replay. All T1 + T2 fixed. |
| Phase 5 | External paid audit | TBD |

## References

- ADR 0028 — CPI interface versioning (`build-spec/decisions/0028-cpi-versioning.md`)
- ADR 0034 — `seeds::program` mandatory constraint
- ADR 0030 — CPI return-data pattern (Pattern A auto-deserialize)
- ADR 0036 — randomness = sha256(signature)
- `build-spec/architecture/security-model.md` — 7 detailed threat scenarios
- `SECURITY.md` — disclosure process
- `sdk/rust/CAVEATS.md` + `sdk/typescript/CAVEATS.md` — maturity disclosures
- `audit/phase-4.5/FINDINGS-CONSOLIDATED.md` — Phase 4.5 detailed findings
