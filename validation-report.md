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
