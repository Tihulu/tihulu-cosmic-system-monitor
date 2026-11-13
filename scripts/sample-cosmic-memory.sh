#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Read-only 10-minute sampler for COSMIC Panel and individual applet PIDs.
# Usage: scripts/sample-cosmic-memory.sh PID [duration_seconds=600] [interval_seconds=10] [output.csv]
set -Eeuo pipefail

if (( $# < 1 || $# > 4 )); then
    printf 'Usage: %s PID [duration_seconds=600] [interval_seconds=10] [output.csv]\n' "$0" >&2
    exit 2
fi

pid="$1"
duration="${2:-600}"
interval="${3:-10}"
output="${4:-cosmic-memory-${pid}.csv}"

for number in "$pid" "$duration" "$interval"; do
    if [[ ! "$number" =~ ^[0-9]+$ ]]; then
        echo "PID, duration and interval must be nonnegative integers" >&2
        exit 2
    fi
done
if (( pid < 1 || duration < 1 || interval < 1 )); then
    echo "PID, duration and interval must be positive" >&2
    exit 2
fi

proc="/proc/$pid"
if [[ ! -r "$proc/smaps_rollup" || ! -r "$proc/stat" || ! -d "$proc/fd" ]]; then
    echo "Cannot read $proc; check PID and access as the same user" >&2
    exit 1
fi

# Start ticks distinguish an applet/panel restart from reuse of a numeric PID.
start_ticks() {
    sed -E 's/^[^)]*\) //' "$proc/stat" | awk '{print $20}'
}
initial_start_ticks="$(start_ticks)"
if [[ -z "$initial_start_ticks" ]]; then
    echo "Could not identify process start time" >&2
    exit 1
fi

# Refuse to overwrite an existing data capture.
if ! (set -C; printf '%s\n' 'timestamp_utc,elapsed_seconds,pid,rss_kib,pss_kib,private_dirty_kib,swap_kib,fd_count,memfd_count' > "$output"); then
    echo "Output already exists or cannot be created: $output" >&2
    exit 1
fi

started="$(date +%s)"
while :; do
    if [[ ! -r "$proc/smaps_rollup" ]] || [[ "$(start_ticks 2>/dev/null || true)" != "$initial_start_ticks" ]]; then
        echo "PID $pid exited/restarted; stopping (CSV preserved): $output" >&2
        exit 1
    fi

    # All KiB values come from the same smaps_rollup snapshot.
    if ! metrics="$(awk '
        /^Rss:/ {rss=$2}
        /^Pss:/ {pss=$2}
        /^Private_Dirty:/ {dirty=$2}
        /^Swap:/ {swap=$2}
        END {printf "%d,%d,%d,%d",rss,pss,dirty,swap}
    ' "$proc/smaps_rollup")"; then
        echo "Failed to read smaps_rollup" >&2
        exit 1
    fi

    fd_count="$(find "$proc/fd" -mindepth 1 -maxdepth 1 -printf 'x' 2>/dev/null | wc -c)"
    memfd_count="$(find "$proc/fd" -mindepth 1 -maxdepth 1 -type l -printf '%l\n' 2>/dev/null | awk '/memfd:/{count++} END{print count+0}')"
    elapsed="$(( $(date +%s) - started ))"
    printf '%s,%s,%s,%s,%s,%s\n' \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$elapsed" "$pid" "$metrics" "$fd_count" "$memfd_count" >> "$output"

    if (( elapsed >= duration )); then
        break
    fi
    remaining="$((duration - elapsed))"
    if (( remaining < interval )); then
        sleep "$remaining"
    else
        sleep "$interval"
    fi
done
echo "Capture complete: $output"
