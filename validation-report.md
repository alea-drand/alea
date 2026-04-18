# Alea Validation Report

## Phase 1.1.A — ark-ff BPF Compilation (2026-04-16)

**Result:** PASS — Attempt 1 (ark-ff 0.5.0 stock)

**Versions:**
- ark-ff = 0.5.0
- ark-bn254 = 0.5.0 (with `features = ["curve"]`, `default-features = false`)
- ark-ec = 0.5.0
- ark-serialize = 0.5.0
- Rust toolchain: stable 1.94.1
- BPF toolchain: 1.89.0-sbpf-solana-v1.52
- Solana CLI: 3.1.13 (Agave)
- Anchor CLI: 0.30.1

**Build command:** `anchor build --no-idl`
**Build time:** ~10 seconds (cold compile)
**Binary size:** 173,120 bytes (with unused imports — will grow with actual usage)

**Native test:** `cargo test -p alea-verifier fq_basic_ops` — PASS
- `Fq::from(42).square()` != `Fq::ZERO` ✓
- `Fq::from(42) * Fq::from(42)` == `Fq::from(42).square()` ✓

**Notes:**
- No feature flag issues. `default-features = false` works out of the box.
- `ark-bn254` requires `features = ["curve"]` to export `Fq` (base field type is behind `#[cfg(feature = "curve")]`)
- `Fq::ZERO` is on the `AdditiveGroup` trait in 0.5.0 (not directly on `Fq`)
- No `parallel` or `asm` features enabled — no threading or x86 intrinsics in BPF build
- `solana-program 1.18.26` (via Anchor) pulls in `ark-bn254 0.4.0` transitively via `light-poseidon`. Two versions coexist without conflict.
- `solana-bn254` crate is NOT needed — `solana_program::alt_bn128` provides pairing/addition/multiplication syscalls

**Anchor IDL note:** `anchor build` (with IDL) fails due to proc-macro2 >= 1.0.95 removing `source_file()`. Fix: `anchor build --no-idl`. No Anchor 0.30.x backport exists (fixed in 0.31.1). For tests, IDL must be manually provided or generated separately.

**Decision impact:**
- ADR 0023: resolved — ship ark-ff 0.5.0 stock (Attempt 1 succeeded)
- OPEN-ITEMS #1: resolved — `ark-ff = "=0.5.0"`
- OPEN-ITEMS #2: resolved — `channel = "stable"` (Rust 1.94.1)
- Attempts 2, 3, 4 NOT needed

---

## Phase 1.1.B — G2 Subgroup Check (2026-04-16)

**Result:** FAIL — Primary path exceeds 1.4M CU. Ship fallback (hardcoded const).

**Test:** `G2Affine::is_in_correct_subgroup_assuming_on_curve()` on BPF localnet.
**CU consumed:** >1,400,000 (hit transaction limit, did not complete)

**Decision impact:**
- ADR 0027: resolved — ship **fallback path** (hardcoded `EXPECTED_EVMNET_G2_PUBKEY` const comparison)
- Error code 6008 `WrongPubkey` is ACTIVE (not 6005 `InvalidG2Point`)
- Key rotation requires program upgrade (not `update_config`)
- OPEN-ITEMS #4: resolved — fallback path

---

## Phase 1.1.D — CU Benchmark (2026-04-16)

**Result:** PASS — CU measured for all field operations + 3 optimization strategies

### Pure BPF (ark-ff 0.5.0 generic)
| Operation | CU | Method |
|-----------|-----|--------|
| `Fq::pow` (p-1)/2 | 679,173 | generic square-and-multiply |
| `Fq::sqrt` (p+1)/4 | 676,554 | generic square-and-multiply |
| `Fq::inverse` | 25,511 | ark-ff internal (Binary GCD) |

### Optimized Approaches (head-to-head comparison)
| Solution | sqrt CU | Non-QR detect CU | Method |
|----------|---------|-------------------|--------|
| **A: G1 decompress syscall** | **643** | **626** | `alt_bn128_g1_decompress` — computes sqrt(x³+3) |
| **B: big_mod_exp syscall** | **996** | N/A | `sol_big_mod_exp(base, (p+1)/4, p)` |
| **C: Addition chain** | **553,650** | N/A | gnark-crypto 300-op chain, pure BPF |

### Key Findings
1. **G1 decompression is a sqrt oracle** — undocumented optimization. `alt_bn128_g1_decompress` internally computes sqrt(x³+3) for BN254. Returns `Err` for non-QR inputs. 643 CU vs 677K CU = **1,053x improvement**.
2. **big_mod_exp syscall works** — computes arbitrary modular exponentiation at 996 CU for 32-byte inputs. 680x cheaper than BPF.
3. **Addition chain implemented but unnecessary** — 553K CU via gnark-crypto's chain. Superseded by syscall solutions.

### Estimated Full SVDW Pipeline
Using Solution A (G1 decompress) for sqrt + syscalls for pairing:
- expand_message_xmd + hash_to_field: ~10K CU
- 2× map_to_point (field ops + G1 decompress): ~15-30K CU
- G1 addition (syscall): ~334 CU
- Pairing check (syscall): ~49K CU
- sha256 randomness: ~5K CU
- **Estimated total: ~80-100K CU** (needs full pipeline measurement to confirm)

**Decision impact:**
- OPEN-ITEMS #5: resolved — CU budget is viable with syscall-based architecture
- SDK default 900K CU is massive overkill; can likely use 200K CU
- The entire SVDW architecture shifts from "pure BPF field arithmetic" to "syscall-assisted"

---

## ARCHITECTURE CHANGE: Syscall-Based SVDW (2026-04-16)

**Discovery:** Three Solana syscalls can replace expensive BPF field arithmetic:

1. `alt_bn128_g1_decompress` (643 CU) — replaces `Fq::sqrt` for sqrt(x³+3)
2. `sol_big_mod_exp` (996 CU) — replaces `Fq::pow` for arbitrary exponents
3. `alt_bn128_pairing` (49K CU) — already planned for BLS verification

**Impact:** Full drand verify estimated at ~80-100K CU instead of ~1.7M CU.
No existing Solana project uses G1 decompression as a sqrt oracle.

**New ADR needed** to document the syscall-based SVDW architecture decision.

---

## Phase 2 — Localnet Integration + CU Benchmark (2026-04-16)

**Result:** PASS — Gate C cleared. Full on-chain verification confirmed.

### Test Results

| Category | Tests | Pass |
|----------|-------|------|
| initialize | 4 P0 | 4/4 |
| verify | 5 P0 + 1 P1 | 6/6 |
| update_config | 2 P0 | 2/2 |
| CPI (cpi-consumer) | 1 P0 + 1 P1 | 2/2 |
| CU benchmark (50 rounds) | 1 P1 | 1/1 |
| **Total** | **12 P0 + 3 P1** | **15/15** |

### CU Distribution (50 consecutive live drand rounds, evmnet)

Measured on `anchor test` localnet. `verify` instruction, stored-bump
config PDA, preInstructions `ComputeBudgetInstruction::set_compute_unit_limit(1_400_000)`:

| Metric | CU |
|--------|----:|
| min    | 404,275 |
| p50    | 407,690 |
| mean   | 407,881 |
| p95    | 412,605 |
| p99    | 415,379 |
| max    | 415,379 |
| stddev | 2,404   |
| variance (% of mean) | 0.59% |

**Acceptance criteria:**
- AC-16: max CU < 1,000,000 → PASS (max 415,379, 41.5% of ceiling)
- AC-17: variance < 20% of mean → PASS (variance 0.59%)

**vs. ADR 0037 prediction (~80-100K CU):** actual 4-5× higher. The
estimate under-counted the cost of remaining ark-ff field ops (SVDW
tv arithmetic, on_curve_g1 check) on BPF. Still well under the 1M
gate and leaves ~984K CU headroom under Solana's 1.4M per-tx ceiling
for consumer logic with the SDK's 900K default.

**Binary sizes (BPF release):**
- `alea_verifier.so`: 295,320 bytes (288 KB) — in 100-600 KB spec band
- `cpi_consumer.so`: 191,952 bytes (187 KB) — test fixture only

**Notable empirical confirmations:**
- ADR 0030 CPI return data: **Pattern A** (Anchor 0.30.x auto-serialize
  of `Result<[u8; 32]>`). `cpi-consumer` successfully reads the 32-byte
  randomness via `alea_verifier::cpi::verify(...)?.get()`. OPEN-ITEMS #3 resolved.
- ADR 0034 `seeds::program` constraint: works. Wrong config PDA derived
  from cpi-consumer's own ID is rejected at the constraint layer
  (Anchor 2xxx), not the custom 6xxx range.
- Error code tri-state handling: 6000 (InvalidSignature on wrong round
  sig), 6001 (InvalidG1Point on non-canonical x=p), 6002 (RoundZero),
  and Anchor 2001 (ConstraintHasOne on wrong authority) all fire as
  specified in `program/spec.md §Error Codes`.

**Gate C cleared.** Phase 2 complete. Full on-chain drand BN254 BLS
verification proven on Solana localnet. Ready for Phase 3 devnet deploy.

Raw per-round CU data: `validation-report-phase2-cu.json` (50 rounds,
not committed to avoid growing the repo — regenerate with
`anchor test --skip-build`).

## Phase 3 — Devnet Deployment + Live Testing (2026-04-18)

**Deployed binary SHA256:** `8965062489fdcdbb538597545fc6692f3f580d770d34f2d42000a70560984b1c` (matches v0.2.0-audit-passed baseline byte-for-byte via `solana program dump`)

**Deployment:**
- Program ID: `ALEAydzHd4cN2EWcdHKp4hehAE4B88b16gqVtVqsck2U`
- Cluster: devnet (https://api.devnet.solana.com + Helius authenticated for reliability)
- Deploy tx: `4tr4yetr4j3U9LjSNfA4CWVYNKrZr9nAKx51pV9baXJ7FAB1Hi4AjquAcQUcpgGD7buiGR9ppSbYTrVETnD9Zdtj`
- ProgramData address: `6GvKJTbq5hggrPwfQuE7vt6tgHn8mn1P9bu63QZ7q9kZ`
- Data Length: 800,000 bytes (`--max-len 800000` — 2.2× headroom for future upgrades)
- Upgrade authority: `9cPWdtoR7sW7VVYxfrJZ9ekxX2fZctskXn3L4BSmafcc` (deployer; pre-Squads per ADR 0009 v1)

**Upgrade fire drill (Phase D-bis):** same audit-passed binary redeployed immediately after initial deploy. Tx `5bbihbgp2CYJ8hq5nJWCRxk5xqxY47JkyXTd4qJiXb48CXbtdCncQhWUVYSi2tUx168JNjc9FJYD6x36BMQGTDut`. Post-upgrade SHA still `8965062489fd...`; authority preserved. Upgrade path proven — the "can always change the code" invariant holds.

**Config PDA initialization:**
- Address: `6anALRxD98Tw7zbA9d5i4NJfTvxDsNBHohHVJWxv2Xm8` (bump 253)
- Init tx: `2ty5NNeHf8PCt7aHiL8adqyEA9VaA5PVFEerEYoc9JTBT6vZ1mCs638PRfwQudornfSht4v8kG2tH5LGXHjdJUPW`
- Wave X+1 on-chain gate accepted deployer signer
- Post-flight byte-equality verified all 6 stored fields (pubkey_g2 / chain_hash / genesis_time / period / authority / bump)

**Live drand round verification (10 rounds, live mode racing beacon emission):**

| Round | CU | Tx sig |
|---|---|---|
| 16341843 | 406,211 | `324ahSPNc89dk8xodyPwK66vLkpSWk58BS3qyfp5q94VY2MWooxTN4gpFu88JLqM8ZG82L6Jy6YT8KCUgMC2mgvx` |
| 16341888 | 409,586 | `5ittXKXF6xyAsX7x3ddWrbfCoEgBKbUK6g79KfacHcfi65cxeC37ouNBKNZZqFvA3JosSXofDLhqDN7LB6qnGWj6` |
| 16341889 | 406,057 | `2MRzSsnYLN8XNB2bNut5Qvq4AmuZotLrzb9abg2z41JTtYGX5mrFBos63aisUgR9jqvuFqgCAzCeRPLHD8bHKuT4` |
| 16341912 | 404,039 | `otNX3H3XtHWJKCUda78pMj32sTWJYqZK6cEhKQgcLFFZdn4YmHmDqwQe1LN2ryAyeKAFVdffPu3wBhVWCuGX4dQ` |
| 16341913 | 406,896 | `4aVHP3ZkHSTR3BhA2r4VLF7FFVF5pqJj4p7aC1GSaA8DEK8kxxhEMfUve8LEtYu9sNaZuLnzYc3Qng9PpFaT2vmz` |
| 16341914 | 406,638 | `3H9vDaAiSM78RSv4mqohadZC8pPsQR8rxV2KJbxwE3XPirrozZEveLfJHqoKiNwYywVRqVuKheSKBnWuhgdhydx6` |
| 16341915 | 406,273 | `7KTwYYLrSEBX8ofSYYBJM29UZxYLZf2Na3M3SH4ctM1CZ9pbemvp8opKYCheaKygFiMoxwqmdEjZAAz9sGGQCTq` |
| 16341916 | 404,448 | `5Chq8o8EtzmJ8qhNxqUqM1hYS5JMvBoLRTBe2oSUAHG6tTVwEjFcyE7o38pjSYtaD8K3syxADhEpXVsvjzUtrkgh` |
| 16341917 | 406,487 | `43HqXoTsaWfmUkXNGh6ga6hExHeCN6Br8tcKsd2ggiZTB4ycjyiy4Qkgueza3vMdfXi9tmE2fSHByowzyEp6QVaG` |
| 16341918 | 409,120 | `iRFjZkgWKpngMDW3H61dKrrsMwMd2ZJhXq4wguUyTnCfhypKffvRKYBfipTt8HS5pqQk2QJYsmp7xWRYWCzBBNd` |

All 10 rounds: `returnData == sha256(signature)` byte-for-byte (ADR 0036) AND matched drand's reported `randomness` field.

**CU distribution (n=7, second batch after retry patch):**

| Metric | Devnet | Localnet (Wave G baseline) |
|---|---|---|
| min | 404,039 | — |
| p50 | 406,487 | — |
| mean | 406,272 | — |
| p95 | 409,120 | — |
| max | **409,120** | **413,874** |
| variance | **0.38%** | **0.53%** |

Devnet is ~1% more efficient than localnet CU-wise — within the ±5% tolerance. Max 409,120 sits at 45% of the 900K SDK gate. Raw distribution: `validation-report-phase3-cu.json`.

**Failure cases on devnet:**

| Case | Expected code | Actual | Explorer |
|---|---|---|---|
| Round 0 | 6002 `RoundZero` | 6002 ✓ | tx sig in `validation-report-phase3-failures.json` |
| Wrong-round pairing mismatch | 6000 `InvalidSignature` | 6000 ✓ | — |
| Off-curve signature (x = Fq prime) | 6001 `InvalidG1Point` | 6001 ✓ | — |

All three branches of the verify pipeline reject as specified on real network.

**SDK unit tests:** 6/6 pass via `cargo test -p alea-sdk is_round_recent`.

**Live-Clock test:** 3/3 recency assertions pass against real devnet Clock sysvar + Config PDA via `cargo test -p alea-sdk --test devnet_clock -- --ignored`. (Sample output: `computed current_round=16342022` at devnet slot 456,465,711 / unix 1,776,547,138.)

**ADR 0031 chain-hash guard framing:** the initialize handler's `require!(chain_hash == EXPECTED_EVMNET_CHAIN_HASH)` guard (error 6007) and companion `EXPECTED_EVMNET_*` byte-equality guards (6008/6010/6011) are validated via (a) Wave G localnet regression tests, (b) production init's post-flight byte-equality of all 6 Config fields. Re-running initialize with a wrong chain_hash on devnet is not feasible (script aborts pre-flight if Config PDA exists; PDA is created on first init); the protocol-level guarantee is that only-correct-constants-can-be-stored, which is satisfied.

**Phase 3 acceptance gate: CLEARED** except §3.8 Randamu outreach (deferred — Aaron drafts manually). Devnet program ID ready for Phase 4 SDK + consumer integration.

**Invariants hold:**
1. Program ID `ALEAydzHd...` live on devnet, owned by `BPFLoaderUpgradeab1e11111111111111111111111` ✓
2. ProgramData authority = `9cPWdtoR...` (deployer) ✓
3. Deployed binary SHA256 matches audit-passed `8965062489fd...` ✓
4. Config PDA exists, 217 B, all 6 fields byte-match hardcoded constants ✓
5. `config.authority` = deployer ✓
6. Upgrade path proven via Phase D-bis fire drill ✓
