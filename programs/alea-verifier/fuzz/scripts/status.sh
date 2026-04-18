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
#
# libFuzzer -fork output format (one line per job merge, every ~5s):
#   [TS] #4910388: cov: 1504 ft: 12889 corp: 1767 exec/s: 17275 oom/timeout/crash: 0/0/0 time: 104s job: 22 dft_time: 0
# Plateau detection for fork mode uses cov-delta-over-time (NEW events aren't
# emitted in fork mode — master only prints aggregated stat lines).
status_target() {
    local target="$1"
    local log
    log=$(ls -t "$LOGS"/"$target"-*.log 2>/dev/null | head -1 || true)
    if [[ -z "$log" ]]; then
        printf "[%-14s] no log yet (run launch.sh first)\n" "$target"
        return
    fi

    # Match any line containing "cov:" — covers fork-mode "#N: cov:" lines AND
    # non-fork "#N pulse cov:" lines.
    local last_stat_line
    last_stat_line=$(grep -E 'cov: [0-9]+ ft: [0-9]+' "$log" | tail -1 || true)
    if [[ -z "$last_stat_line" ]]; then
        printf "[%-14s] log exists but no libFuzzer stats yet (cargo building)\n" "$target"
        return
    fi

    local iters cov ft corp_count exec_s oom_tc
    iters=$(echo "$last_stat_line" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
    cov=$(echo "$last_stat_line" | grep -oE 'cov: [0-9]+' | head -1 | awk '{ print $2 }')
    ft=$(echo "$last_stat_line" | grep -oE 'ft: [0-9]+' | head -1 | awk '{ print $2 }')
    exec_s=$(echo "$last_stat_line" | grep -oE 'exec/s: [0-9]+' | head -1 | awk '{ print $2 }')
    # corp: 1767 (fork) or corp: 156/892Kb (non-fork) — take first number.
    corp_count=$(echo "$last_stat_line" | grep -oE 'corp: [0-9]+' | head -1 | awk '{ print $2 }')
    # oom/timeout/crash (fork mode only, shows child-process failure counts).
    oom_tc=$(echo "$last_stat_line" | grep -oE 'oom/timeout/crash: [0-9]+/[0-9]+/[0-9]+' | head -1 | awk '{ print $2 }')

    # Plateau detection via cov delta. Find the line from ~60 minutes ago and
    # compare its cov value to current.
    local now_ep
    now_ep=$(now_epoch)
    local old_cov="" old_line cov_age_sec="" line_ts_epoch=0

    # Walk log backwards, find the oldest stat line still within our check window
    # (60 min back). Use its cov for delta. If no such line (log <60m old), use
    # first stat line of the log.
    local cutoff_epoch=$(( now_ep - 3600 ))
    old_line=$(grep -E 'cov: [0-9]+ ft: [0-9]+' "$log" | head -1 || true)
    # Scan for most recent line BEFORE cutoff.
    while IFS= read -r line; do
        local ts
        ts=$(echo "$line" | awk '{ print $1 }' | tr -d '[]')
        local ep
        ep=$(to_epoch "$ts")
        if (( ep > 0 && ep <= cutoff_epoch )); then
            old_line="$line"
        fi
    done < <(grep -E 'cov: [0-9]+ ft: [0-9]+' "$log")

    old_cov=$(echo "$old_line" | grep -oE 'cov: [0-9]+' | head -1 | awk '{ print $2 }')
    local old_ts
    old_ts=$(echo "$old_line" | awk '{ print $1 }' | tr -d '[]')
    line_ts_epoch=$(to_epoch "$old_ts")
    if (( line_ts_epoch > 0 )); then
        cov_age_sec=$(( now_ep - line_ts_epoch ))
    fi

    local cov_delta="?"
    if [[ -n "$old_cov" && -n "$cov" ]]; then
        cov_delta=$(( cov - old_cov ))
    fi

    # Corpus file count from disk (ground truth).
    local corpus_disk=0
    if [[ -d "$CORPUS_ROOT/$target" ]]; then
        corpus_disk=$(find "$CORPUS_ROOT/$target" -type f 2>/dev/null | wc -l | tr -d ' ')
    fi

    # Verdict based on cov delta over elapsed window.
    local verdict
    if [[ "$cov_delta" == "?" || -z "$cov_age_sec" ]]; then
        verdict="WARMING"
    elif (( cov_age_sec < 900 )); then
        verdict="GROWING"
    elif (( cov_delta > 0 )); then
        if (( cov_age_sec < 3600 )); then
            verdict="GROWING"
        else
            verdict="SLOWING"
        fi
    else
        # cov_delta == 0
        if (( cov_age_sec >= 3600 )); then
            verdict="PLATEAU"
        else
            verdict="SLOWING"
        fi
    fi

    # Crash check. Fork-mode oom/timeout/crash counter in logs.
    local crashes=0
    if [[ -d "$ARTIFACTS_ROOT/$target" ]]; then
        crashes=$(find "$ARTIFACTS_ROOT/$target" -type f ! -name '.*' 2>/dev/null | wc -l | tr -d ' ')
    fi
    local crash_tag=""
    if (( crashes > 0 )); then
        crash_tag=" CRASHES=$crashes"
    fi
    if [[ -n "$oom_tc" && "$oom_tc" != "0/0/0" ]]; then
        crash_tag="$crash_tag fork-oom/to/crash=$oom_tc"
    fi

    printf "[%-14s] iters=%-10s cov=%-5s ft=%-5s corp=%-4s (disk=%s) exec/s=%-6s cov_Δ%s=%s  -> %s%s\n" \
        "$target" "${iters:-?}" "${cov:-?}" "${ft:-?}" "${corp_count:-?}" "$corpus_disk" "${exec_s:-?}" "$(fmt_age "${cov_age_sec:-0}")" "$cov_delta" "$verdict" "$crash_tag"
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
    echo "Verdict: WARMING <start>, GROWING <15m or cov still rising,"
    echo "         SLOWING >=15m with small growth, PLATEAU >=60m with zero cov delta."
    echo "cov_Δ<window> = edges gained across that window (0 at PLATEAU)."
    echo "Crashes land in $ARTIFACTS_ROOT/<target>/ — inspect with: cargo fuzz tmin <target> <artifact>"
}

main "$@"
