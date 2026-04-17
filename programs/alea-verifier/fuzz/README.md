# Alea Fuzzing Harness — Phase 2.5 Stage 5a

## Why cargo-fuzz, not Trident

Original plan called for Ackee Blockchain's Trident fuzzer (our 24h adversarial input campaign). `trident init` fails in this repo with the **same** `proc-macro2::Span::source_file` incompatibility that forced Alea's hand-written IDL (commit `a74a2e5`):

```
error[E0599]: no method named `source_file` found for struct `proc_macro2::Span`
  --> anchor-syn-0.30.1/src/idl/defined.rs:499:66
```

Trident's first step is `anchor build` to regenerate the IDL — that step is unbuildable on `rustc 1.94.1 + anchor-syn 0.30.1`.

Switched to `cargo-fuzz` (rust-fuzz official, libFuzzer backend — same fuzzing engine Trident wraps under the hood). cargo-fuzz operates on the Rust library crate directly without going through Anchor's IDL build, so the proc-macro2 incompat is bypassed. For Alea's use case — fuzzing the crypto functions (SVDW, hash-to-curve, on-curve, pairing) rather than the Anchor instruction machinery — this gives **better** coverage anyway.

## Tooling

- `cargo-fuzz 0.13.1` (via `cargo install cargo-fuzz --locked`)
- `libfuzzer-sys 0.4` (LLVM libFuzzer Rust bindings)
- `arbitrary 1` (structured input generation)
- Nightly Rust required for sanitizers — `cargo +nightly fuzz ...`

## Targets

### 1. `verify_beacon` — End-to-end pipeline

Input: `(round: u64, signature: [u8; 64])`. Pubkey hardcoded to `EXPECTED_EVMNET_PUBKEY`.

- **Primary invariant:** `verify_beacon` never panics (any panic = libFuzzer crash).
- Expected behavior: 99.999...% of random inputs return `None` (no valid signature via random bytes).
- Finds: any reachable panic, UB, or unwrap() in the E2E verify pipeline.

### 2. `hash_to_g1` — Hash-to-curve isolation

Input: `Round(u64)` OR `RawMessage(Vec<u8>)` (bounded to 1024 bytes).

- **Primary invariant:** every output point is on the BN254 G1 curve (asserted via `on_curve_g1` post-check).
- Specifically targets P02-T1-02 finding: BPF `try_sqrt_curve` returns `Fq::ZERO` for `x=0` vs native `sqrt(3)`. The resulting (0,0) is NOT on curve — if SVDW ever produces this, the on-curve assertion fires.
- Finds: SVDW constants bugs, sgn0 bugs, sign-correction bugs, expand_message bugs.

### 3. `on_curve_g1` — CVE-2025-30147 regression target

Input: random 64 bytes.

- **Primary invariant:** Alea's `on_curve_g1` agrees byte-for-byte with an independent arkworks-based check.
- Directly detects the CVE-2025-30147 Besu bug class: subgroup check ordering bypass. If anyone adds a subgroup check before on-curve, this target catches the divergence immediately.
- Finds: canonical-form bypass, curve-equation bypass, any drift from the arkworks reference.

## Building

```bash
cd programs/alea-verifier
cargo +nightly fuzz build
```

Targets compile to `programs/alea-verifier/fuzz/target/.../release/{verify_beacon,hash_to_g1,on_curve_g1}`.

## Running (Stage 5b — to be started in a fresh session)

**Smoke test (each target, 30s):**

```bash
cd programs/alea-verifier
cargo +nightly fuzz run verify_beacon -- -max_total_time=30
cargo +nightly fuzz run hash_to_g1 -- -max_total_time=30
cargo +nightly fuzz run on_curve_g1 -- -max_total_time=30
```

**24h campaign (Stage 5b — split across targets):**

Three 8h runs, each in its own tmux pane, logged for the GitHub release proof tarball:

```bash
mkdir -p fuzz/run-logs

# Pane 1 — verify_beacon
tmux new -s fuzz-verify -d
tmux send-keys -t fuzz-verify "cd $(pwd) && cargo +nightly fuzz run verify_beacon -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/verify_beacon.log" Enter

# Pane 2 — hash_to_g1
tmux new -s fuzz-hash -d
tmux send-keys -t fuzz-hash "cd $(pwd) && cargo +nightly fuzz run hash_to_g1 -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/hash_to_g1.log" Enter

# Pane 3 — on_curve_g1
tmux new -s fuzz-oncurve -d
tmux send-keys -t fuzz-oncurve "cd $(pwd) && cargo +nightly fuzz run on_curve_g1 -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/on_curve_g1.log" Enter

# Monitor any session:
tmux attach -t fuzz-verify    # detach with Ctrl+B then D

# After 8h each, all sessions exit cleanly. Check:
ls fuzz/artifacts/          # crashes land here if found
grep -c "NEW_FUNC\|cov:" fuzz/run-logs/*.log
```

**Alternative: single 24h sequence (cleaner for release proof)**

```bash
# 8h each, back-to-back. Total 24h.
cargo +nightly fuzz run verify_beacon -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/verify_beacon.log && \
cargo +nightly fuzz run hash_to_g1 -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/hash_to_g1.log && \
cargo +nightly fuzz run on_curve_g1 -- -max_total_time=28800 2>&1 | tee fuzz/run-logs/on_curve_g1.log
```

## Release Proof Package (Stage 5b deliverable)

After the 24h campaign, package these into `fuzzing-campaign-proof.tar.gz`:

- `fuzz/run-logs/*.log` — full stdout/stderr from each target
- `fuzz/artifacts/**` — crash corpus (empty = pass)
- `fuzz/corpus/**` — accumulated interesting inputs (shows coverage depth)
- `campaign-metadata.json` — start time, end time, seed, iteration counts, tool versions
- `environment.txt` — rustc nightly version, cargo-fuzz version, OS (Darwin 25.3.0), M5 Max specs
- `SHA256SUMS` — manifest

## Shutdown

At end of each run:
```bash
tmux kill-session -t fuzz-verify 2>/dev/null
tmux kill-session -t fuzz-hash 2>/dev/null
tmux kill-session -t fuzz-oncurve 2>/dev/null
pkill -f "cargo.*fuzz run" 2>/dev/null
pkill -f libfuzzer 2>/dev/null
```

## Known good baseline (for regression)

Build succeeded 2026-04-16. 3 targets compiled in 39s with ASAN + coverage instrumentation.

Targets **not yet smoke-tested** — deferred to Stage 5b session.

## Stage 5b Parallel Campaign — How to Run (superseded handoff)

The original handoff called for 3×8h sequential runs (24h wall time). That was wasteful on an M5 Max with 16 cores. This section documents the **current launch flow**: 3 targets running in parallel, each with `-fork=3`, totaling 9 concurrent libFuzzer processes. ~4h wall time, ~36 CPU-fuzz-hours.

### One-command launch

```bash
cd /Users/aaron/dev/work/alea/programs/alea-verifier/fuzz
bash scripts/launch.sh --dry-run      # validate env + build + corpus seed
bash scripts/launch.sh --smoke-only   # 30s × 3 targets, fail fast on crash
bash scripts/launch.sh                # full campaign: smoke + tmux 3-pane 4h
```

### What `launch.sh` does

1. Exports Solana CLI + cargo on PATH
2. Verifies nightly toolchain + cargo-fuzz installed
3. `cargo +nightly fuzz build` (cached after first run)
4. Seeds `verify_beacon` corpus with 10 real drand beacons + 15 adversarial cases via `scripts/seed-corpus.py` (72-byte `arbitrary` layout)
5. Smoke tests each of the 3 targets for 30s in foreground, aborts on any crash
6. Creates tmux session `alea-fuzz` with 3 horizontal panes
7. Launches one target per pane:
   ```
   cargo +nightly fuzz run <target> -- \
       -max_total_time=14400 -fork=3 -use_value_profile=1 \
       -reduce_inputs=1 -print_final_stats=1 \
       -artifact_prefix=fuzz/artifacts/<target>/ \
       [-max_len=1024]
   ```
8. Each pane's output is timestamped (`[YYYY-MM-DDTHH:MM:SSZ]` prefix) and teed to `run-logs/<target>-<UTC>.log`

### Watching live

```bash
tmux attach -t alea-fuzz     # watch all 3 panes
# Ctrl-B then ↑/↓           switch focus between panes
# Ctrl-B then z             zoom current pane to full size (toggle)
# Ctrl-B then d             detach (campaign keeps running)
# Ctrl-C in a pane          stop that target early
tmux kill-session -t alea-fuzz   # emergency stop
```

### On-demand status check

```bash
bash scripts/status.sh
```

Prints one line per target with iteration count, coverage edges, features, corpus size, exec/s, time since last NEW-coverage event, and a verdict (`GROWING` / `SLOWING` / `PLATEAU`). Also reports any crash artifacts.

### Plateau heuristic (when to Ctrl-C early)

Verdict thresholds:
- **GROWING**: <15 min since last NEW-coverage event. Keep running.
- **SLOWING**: 15-60 min since last NEW. Still productive but marginal.
- **PLATEAU**: ≥60 min since last NEW. Coverage has stabilized; early stop is reasonable.

Expected plateau per target (isolated crypto functions on seeded corpora):
- `on_curve_g1` ~<1h (trivial check)
- `hash_to_g1` ~1-2h
- `verify_beacon` ~1.5-2.5h with seeding (vs 3-4h without)

If all 3 hit PLATEAU well before the 4h cap, safe to Ctrl-C remaining panes and proceed to proof packaging.

### Post-campaign packaging

```bash
bash scripts/package-proof.sh
```

Produces `fuzzing-campaign-proof-<UTC>.tar.gz` containing:
- `run-logs/*.log` — full timestamped stdout per target
- `artifacts/**` — crash reproducers (should be empty for a passing run)
- `corpus/**` — accumulated coverage-maximizing inputs
- `coverage/<target>/*.html` — per-target source coverage reports from `cargo fuzz coverage`
- `campaign-metadata.json` — iteration counts, cov/ft totals, crash counts per target
- `environment.txt` — rustc nightly version, cargo-fuzz version, CPU, RAM, flag set

Tarball + `.sha256` get uploaded as GitHub release assets for Alea's public audit artifact set.

### Why these flags

- **`-fork=3`** — libFuzzer's native parallel-workers mode. 3 child processes per target share a corpus dir. Classic ensemble fuzzing; more diverse exploration than one long-running process.
- **`-use_value_profile=1`** — tracks integer comparisons (e.g. `if x == 0x123...`) so the fuzzer can solve magic-number branches that random mutation would miss. Essential for crypto constants.
- **`-reduce_inputs=1`** — on new-coverage finds, libFuzzer shrinks the reproducer to the minimal input. Cleaner crash artifacts, smaller corpus.
- **`-max_len=1024`** — caps arbitrary-input length for `verify_beacon` and `hash_to_g1`. `on_curve_g1` takes fixed 64 bytes and doesn't need this.
- **`-artifact_prefix=fuzz/artifacts/<target>/`** — per-target crash dir keeps artifacts separable across the 3 parallel panes.

### Why corpus seeding matters

`verify_beacon`'s input is 72 bytes: 8-byte LE u64 round + 64-byte signature. Pure random 64 bytes have a 2⁻²⁵⁶ chance of encoding a valid BLS signature, so without seeding the fuzzer rejects 100% of inputs at the on-curve check and never exercises the pairing path. Seeding with 10 real beacons means every iteration reaches pairing; seeding with the 15 adversarial cases from `testing/fixtures/adversarial.json` exercises the full rejection-code table (6000-6009). Cuts effective plateau time roughly in half for `verify_beacon`.

### Scripts reference

| Script | Purpose |
|--------|---------|
| `scripts/launch.sh` | Entry point: validates env, builds, seeds, smokes, launches tmux |
| `scripts/seed-corpus.py` | Writes 25 `.bin` seed files for `verify_beacon` corpus |
| `scripts/status.sh` | On-demand plateau check across all 3 targets |
| `scripts/package-proof.sh` | Assembles the release-proof tarball post-campaign |
