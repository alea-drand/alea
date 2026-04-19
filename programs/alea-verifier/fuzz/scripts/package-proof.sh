#!/usr/bin/env bash
# Build the Stage 5b release-proof tarball after a fuzz campaign completes.
#
# Packages:
#   - fuzz/run-logs/*.log                 (per-target timestamped stdout)
#   - fuzz/artifacts/**                   (crash reproducers — ideally empty)
#   - fuzz/corpus/**                      (accumulated coverage corpus)
#   - fuzz/coverage/<target>/*.html       (per-target HTML coverage reports)
#   - campaign-metadata.json              (start/end, iterations, tool versions)
#   - environment.txt                     (rustc, cargo-fuzz, OS, CPU)
#   - SHA256SUMS                          (hash manifest of the archive contents)
#
# Output:
#   fuzz/fuzzing-campaign-proof-<UTC>.tar.gz  next to fuzz/

set -euo pipefail

# Resolve repo root relative to this script's location (portable across
# developer machines + CI runners) with an env override for CI.
REPO="${ALEA_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)}"
FUZZ_DIR="$REPO/programs/alea-verifier/fuzz"
PROGRAM_DIR="$REPO/programs/alea-verifier"
LOGS="$FUZZ_DIR/run-logs"
ARTIFACTS="$FUZZ_DIR/artifacts"
CORPUS="$FUZZ_DIR/corpus"
COVERAGE="$FUZZ_DIR/coverage"
OUT_DIR="$FUZZ_DIR"

TARGETS=(verify_beacon hash_to_g1 on_curve_g1)

export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"

log() { printf "[package-proof] %s\n" "$*"; }
die() { printf "[package-proof] ERROR: %s\n" "$*" >&2; exit 1; }

[[ -d "$LOGS" ]] || die "no run-logs dir at $LOGS"
[[ $(ls -1 "$LOGS" 2>/dev/null | wc -l) -gt 0 ]] || die "run-logs dir empty"

log "generating per-target coverage HTML..."
mkdir -p "$COVERAGE"
cd "$PROGRAM_DIR"
for t in "${TARGETS[@]}"; do
    log "  cargo fuzz coverage $t ..."
    if cargo +nightly fuzz coverage "$t" 2>&1 | tail -5; then
        log "  coverage data written for $t"
    else
        log "  warning: coverage for $t failed (continuing)"
    fi
done
# cargo-fuzz puts coverage data under fuzz/coverage/<target>/. Leave as-is.

log "extracting campaign stats from logs..."
TMP_META=$(mktemp)
trap 'rm -f "$TMP_META"' EXIT
{
    echo "{"
    echo "  \"generated_at_utc\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
    echo "  \"host\": \"$(hostname)\","
    echo "  \"targets\": ["
    local_first=1
    for t in "${TARGETS[@]}"; do
        latest_log=$(ls -t "$LOGS/$t"-*.log 2>/dev/null | head -1 || true)
        if [[ -z "$latest_log" ]]; then continue; fi

        first_line=$(head -1 "$latest_log")
        # Match any line with 'cov: N ft: N' — covers fork-mode (#N: cov: ...)
        # AND non-fork (#N pulse cov: ...) output formats.
        last_stat=$(grep -E 'cov: [0-9]+ ft: [0-9]+' "$latest_log" | tail -1 || echo "")
        iters=$(echo "$last_stat" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
        cov=$(echo "$last_stat" | grep -oE 'cov: [0-9]+' | head -1 | awk '{ print $2 }')
        ft=$(echo "$last_stat" | grep -oE 'ft: [0-9]+' | head -1 | awk '{ print $2 }')
        corp_pair=$(echo "$last_stat" | grep -oE 'corp: [0-9]+(/[0-9a-zA-Z]+)?' | head -1 | awk '{ print $2 }')
        start_ts=$(echo "$first_line" | awk '{ print $1 }' | tr -d '[]')
        end_ts=$(echo "$last_stat" | awk '{ print $1 }' | tr -d '[]')
        crashes=$(find "$ARTIFACTS/$t" -type f ! -name '.*' 2>/dev/null | wc -l | tr -d ' ')
        corpus_disk=$(find "$CORPUS/$t" -type f 2>/dev/null | wc -l | tr -d ' ')

        [[ "$local_first" == "0" ]] && echo ","
        local_first=0
        cat <<EOF
    {
      "target": "$t",
      "log_file": "$(basename "$latest_log")",
      "start_utc": "$start_ts",
      "end_utc":   "$end_ts",
      "iterations": "${iters:-unknown}",
      "final_coverage_edges": "${cov:-unknown}",
      "final_features": "${ft:-unknown}",
      "final_corpus": "${corp_pair:-unknown}",
      "disk_corpus_count": $corpus_disk,
      "crash_artifacts": $crashes
    }
EOF
    done
    echo "  ]"
    echo "}"
} > "$TMP_META"

cp "$TMP_META" "$FUZZ_DIR/campaign-metadata.json"
log "wrote $FUZZ_DIR/campaign-metadata.json"

log "recording environment..."
{
    echo "=== Alea Phase 2.5 Stage 5b Fuzz Campaign ==="
    echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo ""
    echo "Host: $(hostname)"
    echo "OS: $(uname -sr)"
    echo "CPU: $(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)"
    echo "Cores: $(sysctl -n hw.ncpu 2>/dev/null || getconf _NPROCESSORS_ONLN)"
    echo "RAM: $(sysctl -n hw.memsize 2>/dev/null | awk '{ printf "%.0f GB\n", $1/1024/1024/1024 }' || echo unknown)"
    echo ""
    echo "Rust toolchain:"
    rustup show active-toolchain 2>/dev/null || true
    rustc --version
    echo "cargo-fuzz: $(cargo fuzz --version 2>&1 | head -1)"
    echo ""
    echo "Fuzz flags (full campaign):"
    echo "  -max_total_time=14400  -fork=3  -use_value_profile=1  -reduce_inputs=1"
    echo "  -print_final_stats=1   -artifact_prefix=fuzz/artifacts/<target>/"
    echo "  -max_len=1024 (verify_beacon + hash_to_g1 only)"
} > "$FUZZ_DIR/environment.txt"
log "wrote $FUZZ_DIR/environment.txt"

TS=$(date -u +%Y%m%dT%H%M%SZ)
TARBALL="$OUT_DIR/fuzzing-campaign-proof-$TS.tar.gz"

log "assembling tarball: $TARBALL"
cd "$FUZZ_DIR"

# Manifest relative paths (tar uses them as stored names).
MANIFEST=(
    "run-logs"
    "artifacts"
    "corpus"
    "coverage"
    "campaign-metadata.json"
    "environment.txt"
)

# Filter any missing entries gracefully.
EXISTING=()
for m in "${MANIFEST[@]}"; do
    if [[ -e "$m" ]]; then
        EXISTING+=("$m")
    else
        log "note: $m missing, skipping"
    fi
done

tar -czf "$TARBALL" "${EXISTING[@]}"

log "computing SHA256SUMS..."
(
    cd "$FUZZ_DIR"
    shasum -a 256 "$(basename "$TARBALL")" > "$TARBALL.sha256"
)

SIZE=$(ls -lh "$TARBALL" | awk '{ print $5 }')
SHA=$(cat "$TARBALL.sha256" | awk '{ print $1 }')

log "=================================================================="
log "Package complete:"
log "  Tarball: $TARBALL ($SIZE)"
log "  SHA256:  $SHA"
log "  Meta:    $FUZZ_DIR/campaign-metadata.json"
log "  Env:     $FUZZ_DIR/environment.txt"
log "=================================================================="
log "Next: review run-logs/ for any anomalies, commit a completion report"
log "      locally, and if this is a release-gated campaign, upload the"
log "      tarball as a GitHub release asset (tarball stays gitignored)."
