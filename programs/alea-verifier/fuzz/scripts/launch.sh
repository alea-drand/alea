#!/usr/bin/env bash
# Launch the Alea Phase 2.5 Stage 5b fuzz campaign.
#
# Modes:
#   --dry-run      Validate environment + build, no fuzzing.
#   --smoke-only   30s per target, foreground, fail fast on any crash.
#   (no flag)      Full campaign: smoke test + corpus seed + tmux 3-pane,
#                  each target runs 4h with -fork=3.
#
# Safety:
#   - Exits non-zero on any error (set -euo pipefail).
#   - Smoke test catches panics before the 4h burn.
#   - Artifact dirs are created per target so -artifact_prefix works cleanly.
#   - Will refuse to overwrite an existing alea-fuzz tmux session.

set -euo pipefail

MODE="full"
case "${1-}" in
    --dry-run)    MODE="dry-run" ;;
    --smoke-only) MODE="smoke-only" ;;
    "")           MODE="full" ;;
    *)            echo "Usage: $0 [--dry-run | --smoke-only]" >&2; exit 2 ;;
esac

REPO="/Users/aaron/dev/work/alea"
FUZZ_DIR="$REPO/programs/alea-verifier/fuzz"
PROGRAM_DIR="$REPO/programs/alea-verifier"
SCRIPTS="$FUZZ_DIR/scripts"
LOGS="$FUZZ_DIR/run-logs"
ARTIFACTS="$FUZZ_DIR/artifacts"
CORPUS_ROOT="$FUZZ_DIR/corpus"

TARGETS=(verify_beacon hash_to_g1 on_curve_g1)
TMUX_SESSION="${TMUX_SESSION:-alea-fuzz}"
MAX_TOTAL_TIME="${MAX_TOTAL_TIME:-14400}"    # 4h per target; override via env for pilot runs
SMOKE_TIME="${SMOKE_TIME:-30}"               # seconds per target in smoke test
FORK_WORKERS="${FORK_WORKERS:-3}"
SKIP_SMOKE="${SKIP_SMOKE:-0}"                # set to 1 to skip smoke on re-launches after pilot

# Solana CLI + cargo binaries need to be on PATH for cargo fuzz.
export PATH="$HOME/.local/share/solana/install/active_release/bin:$HOME/.cargo/bin:$PATH"

log() { printf "[launch] %s\n" "$*"; }
die() { printf "[launch] ERROR: %s\n" "$*" >&2; exit 1; }

check_tooling() {
    command -v cargo >/dev/null || die "cargo not on PATH"
    command -v tmux  >/dev/null || die "tmux not on PATH"
    command -v awk   >/dev/null || die "awk not on PATH"
    command -v python3 >/dev/null || die "python3 not on PATH"

    if ! rustup toolchain list 2>/dev/null | grep -q nightly; then
        die "nightly toolchain missing — run: rustup toolchain install nightly"
    fi

    if ! cargo fuzz --version >/dev/null 2>&1; then
        die "cargo-fuzz missing — run: cargo install cargo-fuzz --locked"
    fi

    log "tooling ok: $(cargo --version | head -1), $(tmux -V), $(python3 --version)"
    log "cargo-fuzz: $(cargo fuzz --version 2>&1 | head -1)"
}

build_targets() {
    log "building 3 fuzz targets with nightly..."
    cd "$PROGRAM_DIR"
    cargo +nightly fuzz build 2>&1 | tail -20
    local bindir
    bindir=$(find "$FUZZ_DIR/target" -type d -name release | head -1 || true)
    [[ -n "$bindir" ]] || die "build succeeded but no release binary dir found"
    for t in "${TARGETS[@]}"; do
        [[ -x "$bindir/$t" ]] || die "missing binary: $bindir/$t"
    done
    log "all 3 binaries present in $bindir"
}

seed_corpus() {
    log "seeding verify_beacon corpus from fixtures..."
    python3 "$SCRIPTS/seed-corpus.py"
    local n
    n=$(find "$CORPUS_ROOT/verify_beacon" -type f -name '*.bin' 2>/dev/null | wc -l | tr -d ' ')
    log "corpus/verify_beacon now has $n seed files"
    (( n >= 20 )) || die "corpus seed count $n < 20; something wrong with seeder"
}

prepare_dirs() {
    mkdir -p "$LOGS"
    for t in "${TARGETS[@]}"; do
        mkdir -p "$ARTIFACTS/$t" "$CORPUS_ROOT/$t"
    done
}

# Build libFuzzer flag list for a given target.
flags_for() {
    local target="$1"
    local flags=(
        "-max_total_time=$MAX_TOTAL_TIME"
        "-fork=$FORK_WORKERS"
        "-use_value_profile=1"
        "-reduce_inputs=1"
        "-print_final_stats=1"
        "-artifact_prefix=$ARTIFACTS/$target/"
    )
    if [[ "$target" != "on_curve_g1" ]]; then
        flags+=("-max_len=1024")
    fi
    printf '%s\n' "${flags[@]}"
}

flags_for_smoke() {
    local target="$1"
    # Smoke = no fork, short time — simplest sanity check.
    local flags=(
        "-max_total_time=$SMOKE_TIME"
        "-artifact_prefix=$ARTIFACTS/$target/"
    )
    if [[ "$target" != "on_curve_g1" ]]; then
        flags+=("-max_len=1024")
    fi
    printf '%s\n' "${flags[@]}"
}

smoke_test_one() {
    local target="$1"
    log "SMOKE: $target for ${SMOKE_TIME}s..."
    cd "$PROGRAM_DIR"
    # Build flags inline (macOS bash 3.2 has no mapfile).
    local flags=(
        "-max_total_time=$SMOKE_TIME"
        "-artifact_prefix=$ARTIFACTS/$target/"
    )
    if [[ "$target" != "on_curve_g1" ]]; then
        flags+=("-max_len=1024")
    fi
    # Run to completion; capture exit code.
    if ! cargo +nightly fuzz run "$target" -- "${flags[@]}" 2>&1 | tail -40; then
        die "SMOKE FAILED for $target — inspect $ARTIFACTS/$target/"
    fi
    local crashes
    crashes=$(find "$ARTIFACTS/$target" -type f ! -name '.*' 2>/dev/null | wc -l | tr -d ' ')
    (( crashes == 0 )) || die "smoke test for $target produced $crashes crash artifact(s)"
    log "SMOKE ok: $target (0 crashes)"
}

launch_tmux() {
    if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
        die "tmux session '$TMUX_SESSION' already exists — kill it first: tmux kill-session -t $TMUX_SESSION"
    fi

    log "creating tmux session '$TMUX_SESSION' with 3 horizontal panes..."

    local ts
    ts=$(date -u +%Y%m%dT%H%M%SZ)
    local pane_scripts_dir="$FUZZ_DIR/run-logs/.pane-scripts-$ts"
    mkdir -p "$pane_scripts_dir"

    # Write a self-contained pane script per target. Avoids send-keys
    # long-string issues (Enter can land before a 900-char string
    # finishes typing in zsh).
    write_pane_script() {
        local target="$1"
        local logfile="$LOGS/${target}-${ts}.log"
        local script="$pane_scripts_dir/${target}.sh"
        local flags_str
        flags_str=$(flags_for "$target" | tr '\n' ' ')
        cat > "$script" <<SCRIPT_EOF
#!/usr/bin/env bash
# Auto-generated pane script for $target
export PATH="\$HOME/.local/share/solana/install/active_release/bin:\$HOME/.cargo/bin:\$PATH"
cd "$PROGRAM_DIR"
echo "[launch] starting $target at \$(date -u +%FT%TZ)"
echo "[launch] flags: $flags_str"
echo ""
cargo +nightly fuzz run $target -- $flags_str 2>&1 \\
    | perl -MPOSIX -ne 'BEGIN { \$| = 1 } print "[", POSIX::strftime("%Y-%m-%dT%H:%M:%SZ", gmtime), "] ", \$_' \\
    | tee "$logfile"
echo ""
echo "[DONE] $target finished at \$(date -u +%FT%TZ)"
echo "Log: $logfile"
echo "Run: bash $SCRIPTS/status.sh  for summary"
echo "Ctrl-B d to detach, or exit this shell to close the pane"
exec \${SHELL:-zsh}
SCRIPT_EOF
        chmod +x "$script"
        echo "$script"
    }

    local script0 script1 script2
    script0=$(write_pane_script "${TARGETS[0]}")
    script1=$(write_pane_script "${TARGETS[1]}")
    script2=$(write_pane_script "${TARGETS[2]}")

    # Create the session. First pane is active.
    tmux new-session -d -s "$TMUX_SESSION" -x 220 -y 80 -c "$PROGRAM_DIR"
    # Send short command — reliable regardless of length.
    tmux send-keys -t "$TMUX_SESSION" "bash $script0" Enter
    log "pane 0 launched: ${TARGETS[0]} -> $LOGS/${TARGETS[0]}-${ts}.log"

    tmux split-window -v -t "$TMUX_SESSION" -c "$PROGRAM_DIR"
    tmux send-keys -t "$TMUX_SESSION" "bash $script1" Enter
    log "pane 1 launched: ${TARGETS[1]} -> $LOGS/${TARGETS[1]}-${ts}.log"

    tmux split-window -v -t "$TMUX_SESSION" -c "$PROGRAM_DIR"
    tmux send-keys -t "$TMUX_SESSION" "bash $script2" Enter
    log "pane 2 launched: ${TARGETS[2]} -> $LOGS/${TARGETS[2]}-${ts}.log"

    tmux select-layout -t "$TMUX_SESSION" even-vertical

    log ""
    log "=================================================================="
    log "Campaign is live. To watch:"
    log "    tmux attach -t $TMUX_SESSION"
    log ""
    log "To check status from any other terminal:"
    log "    bash $SCRIPTS/status.sh"
    log ""
    log "To tail one target's log:"
    log "    tail -f $LOGS/<target>-${ts}.log"
    log ""
    log "To detach while attached:  Ctrl-B then d"
    log "To stop one target early:  focus that pane, press Ctrl-C"
    log "To kill everything:        tmux kill-session -t $TMUX_SESSION"
    log "=================================================================="
}

# Always run these regardless of mode (they're safe + idempotent):
check_tooling
prepare_dirs
build_targets

if [[ "$MODE" == "dry-run" ]]; then
    seed_corpus
    log "DRY-RUN OK: tooling verified, binaries built, corpus seeded."
    log "Re-run without --dry-run for full campaign, or --smoke-only for 30s sanity."
    exit 0
fi

seed_corpus

# Fresh artifacts from any previous smoke run so we can distinguish new crashes.
for t in "${TARGETS[@]}"; do
    rm -f "$ARTIFACTS/$t"/* 2>/dev/null || true
done

if [[ "$SKIP_SMOKE" == "1" ]]; then
    log "SKIP_SMOKE=1 set — skipping smoke test"
else
    log "running smoke test (${SMOKE_TIME}s per target)..."
    for t in "${TARGETS[@]}"; do
        smoke_test_one "$t"
    done
    log "all 3 smoke tests passed, 0 crash artifacts"
fi

if [[ "$MODE" == "smoke-only" ]]; then
    log "SMOKE-ONLY: exiting before full campaign launch."
    exit 0
fi

# Fresh artifacts before the real run (smoke leaves them clean, but be explicit).
for t in "${TARGETS[@]}"; do
    rm -f "$ARTIFACTS/$t"/* 2>/dev/null || true
done

launch_tmux
