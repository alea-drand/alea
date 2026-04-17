#!/usr/bin/env bash
# On-demand plateau check for the alea-fuzz campaign.
# Prints one status line per target by parsing the newest run-log.
# Zero dependencies beyond awk/grep/date (macOS or GNU).
#
# Log lines are prefixed with [YYYY-MM-DDTHH:MM:SS] by launch.sh (via awk).
# Example libFuzzer line we parse:
#   [2026-04-16T15:30:12] #1024 pulse cov: 2104 ft: 2341 exec/s: 8200 rss: 45Mb corp: 156/892Kb
# NEW-coverage events use status word "NEW" (or "INITED" first time):
#   [2026-04-16T15:31:45] #1248 NEW   cov: 2105 ft: 2343 ...

set -euo pipefail

FUZZ_DIR="/Users/aaron/dev/work/alea/programs/alea-verifier/fuzz"
LOGS="$FUZZ_DIR/run-logs"
CORPUS_ROOT="$FUZZ_DIR/corpus"
ARTIFACTS_ROOT="$FUZZ_DIR/artifacts"
TARGETS=(verify_beacon hash_to_g1 on_curve_g1)

# Detect date flavor (macOS BSD vs GNU).
if date --version >/dev/null 2>&1; then
    DATE_FLAVOR="gnu"
else
    DATE_FLAVOR="bsd"
fi

# Parse ISO timestamp like 2026-04-16T15:30:12 into epoch seconds.
to_epoch() {
    local ts="$1"
    if [[ "$DATE_FLAVOR" == "gnu" ]]; then
        date -d "$ts" +%s 2>/dev/null || echo 0
    else
        date -j -u -f "%Y-%m-%dT%H:%M:%S" "$ts" +%s 2>/dev/null || echo 0
    fi
}

now_epoch() {
    if [[ "$DATE_FLAVOR" == "gnu" ]]; then
        date -u +%s
    else
        date -u +%s
    fi
}

# Human-friendly age like "14m" or "2h14m" from a seconds integer.
fmt_age() {
    local s="$1"
    if (( s < 60 )); then
        echo "${s}s"
    elif (( s < 3600 )); then
        echo "$((s / 60))m"
    else
        echo "$((s / 3600))h$(((s % 3600) / 60))m"
    fi
}

# Print status for one target.
status_target() {
    local target="$1"
    local log
    log=$(ls -t "$LOGS"/"$target"-*.log 2>/dev/null | head -1 || true)
    if [[ -z "$log" ]]; then
        printf "[%-14s] no log yet (run launch.sh first)\n" "$target"
        return
    fi

    # Last pulse/INITED/NEW/DONE line (anything with "cov: " in it).
    local last_stat_line
    last_stat_line=$(grep -E ' (INITED|NEW|pulse|DONE|RELOAD) +cov: ' "$log" | tail -1 || true)
    if [[ -z "$last_stat_line" ]]; then
        printf "[%-14s] log exists but no libFuzzer stats yet\n" "$target"
        return
    fi

    # Extract fields via awk — tolerate spacing variance.
    local iters cov ft corp_count corp_bytes exec_s
    iters=$(echo "$last_stat_line" | awk '{ for (i=1;i<=NF;i++) if ($i ~ /^#[0-9]+$/) { print substr($i,2); exit } }')
    cov=$(echo "$last_stat_line" | awk '{ for (i=1;i<=NF;i++) if ($i=="cov:") { print $(i+1); exit } }')
    ft=$(echo "$last_stat_line" | awk '{ for (i=1;i<=NF;i++) if ($i=="ft:") { print $(i+1); exit } }')
    exec_s=$(echo "$last_stat_line" | awk '{ for (i=1;i<=NF;i++) if ($i=="exec/s:") { print $(i+1); exit } }')
    local corp_pair
    corp_pair=$(echo "$last_stat_line" | awk '{ for (i=1;i<=NF;i++) if ($i=="corp:") { print $(i+1); exit } }')
    corp_count=${corp_pair%%/*}
    corp_bytes=${corp_pair##*/}

    # Last NEW-coverage event timestamp.
    local last_new_ts_raw last_new_epoch age
    last_new_ts_raw=$(grep -E ' NEW +cov: ' "$log" | tail -1 | awk '{ print $1 }' | tr -d '[]' || true)
    if [[ -z "$last_new_ts_raw" ]]; then
        # No NEW line yet; use INITED line or log start time as origin.
        last_new_ts_raw=$(head -1 "$log" | awk '{ print $1 }' | tr -d '[]' || true)
    fi
    last_new_epoch=$(to_epoch "$last_new_ts_raw")
    local now_ep
    now_ep=$(now_epoch)
    if (( last_new_epoch > 0 )); then
        age=$(( now_ep - last_new_epoch ))
    else
        age=0
    fi

    # Corpus file count from disk (ground truth).
    local corpus_disk=0
    if [[ -d "$CORPUS_ROOT/$target" ]]; then
        corpus_disk=$(find "$CORPUS_ROOT/$target" -type f 2>/dev/null | wc -l | tr -d ' ')
    fi

    # Verdict.
    local verdict
    if (( age < 900 )); then
        verdict="GROWING"
    elif (( age < 3600 )); then
        verdict="SLOWING"
    else
        verdict="PLATEAU"
    fi

    # Crash check.
    local crashes=0
    if [[ -d "$ARTIFACTS_ROOT/$target" ]]; then
        crashes=$(find "$ARTIFACTS_ROOT/$target" -type f ! -name '.*' 2>/dev/null | wc -l | tr -d ' ')
    fi
    local crash_tag=""
    if (( crashes > 0 )); then
        crash_tag=" CRASHES=$crashes"
    fi

    printf "[%-14s] iters=%-10s cov=%-6s ft=%-6s corp=%-4s (disk=%s) exec/s=%-7s last_NEW=%-7s  -> %s%s\n" \
        "$target" "${iters:-?}" "${cov:-?}" "${ft:-?}" "${corp_count:-?}" "$corpus_disk" "${exec_s:-?}" "$(fmt_age "$age")" "$verdict" "$crash_tag"
}

main() {
    if [[ ! -d "$LOGS" ]]; then
        echo "no run-logs directory yet ($LOGS); start campaign first"
        exit 1
    fi

    echo "== alea-fuzz status ($(date -u +%Y-%m-%dT%H:%M:%SZ)) =="
    for t in "${TARGETS[@]}"; do
        status_target "$t"
    done
    echo ""
    echo "Verdict thresholds: GROWING <15m since NEW | SLOWING 15-60m | PLATEAU >=60m"
    echo "  Crashes land in $ARTIFACTS_ROOT/<target>/ — inspect with: cargo fuzz tmin <target> <artifact>"
}

main "$@"
