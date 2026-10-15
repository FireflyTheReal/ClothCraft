#!/bin/sh
# Reproducible CPU baseline matrix for docs/PERFORMANCE.md (Round 1, measurement only).
#
# Runs the shipped CLI scenes on the CPU VBD + OGC backend at three mesh
# densities, and records per-run wall time and peak RSS. Raw per-step metrics
# stay in each output directory; summarise them with
# `scripts/perf/summarize_metrics.py`.
#
# Usage:
#   BIN=target/release/cloth-fit STEPS=180 THREADS=2 scripts/perf/run_cpu_matrix.sh
#
# `--garment` accepts only tshirt and dress: the apron scenes referenced by the
# validation docs are private and are not in this repository.
set -eu

BIN=${BIN:-target/release/cloth-fit}
OUT=${OUT:-output/perf/cpu}
STEPS=${STEPS:-180}
THREADS=${THREADS:-2}

mkdir -p "$OUT"
echo "run,binary,steps,threads,wall_s,user_s,sys_s,max_rss_bytes,load_1m" > "$OUT/index.csv"

for garment in tshirt dress; do
  for mm in 40 30 25; do
    dir="$OUT/$garment-$mm"
    mkdir -p "$dir"
    load=$(uptime | sed 's/.*averages: //; s/,.*//')
    echo "==> $garment @ ${mm}mm, $STEPS steps, $THREADS threads (load 1m: $load)"
    /usr/bin/time -l "$BIN" --garment "$garment" --mesh-mm "$mm" \
      --steps "$STEPS" --threads "$THREADS" --output "$dir" \
      > "$dir/stdout.log" 2> "$dir/stderr.log"
    # BSD `time -l` summary line: "  <real> real  <user> user  <sys> sys"
    set -- $(awk '/ real /{print $1, $3, $5}' "$dir/stderr.log")
    real=${1:-}; user=${2:-}; sys=${3:-}
    rss=$(awk '/maximum resident set size/{print $1}' "$dir/stderr.log")
    echo "$garment-$mm,$BIN,$STEPS,$THREADS,$real,$user,$sys,${rss:-},${load:-}" >> "$OUT/index.csv"
    echo "    real=${real:-?}s user=${user:-?}s sys=${sys:-?}s max_rss=${rss:-?}B"
  done
done
echo "==> wrote $OUT/index.csv"
