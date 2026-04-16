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
