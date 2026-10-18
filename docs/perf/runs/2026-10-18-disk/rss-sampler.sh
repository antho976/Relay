#!/bin/bash
# Sample every 0.5 s the summed and largest RSS (MB) of rustc and linker processes whose
# working directory is under $1, until the file $2 exists. Prints "peak_sum_mb peak_single_mb".
root=$1; stop=$2; peak_sum=0; peak_one=0
while [ ! -e "$stop" ]; do
  sum=0; one=0
  for p in $(pgrep -x 'rustc|ld.lld|ld|cc|clang|ld.mold' 2>/dev/null); do
    cwd=$(readlink /proc/$p/cwd 2>/dev/null) || continue
    case "$cwd" in "$root"*) ;; *) continue;; esac
    kb=$(awk '/^VmRSS/{print $2}' /proc/$p/status 2>/dev/null)
    [ -n "$kb" ] || continue
    sum=$((sum + kb)); [ $kb -gt $one ] && one=$kb
  done
  [ $sum -gt $peak_sum ] && peak_sum=$sum
  [ $one -gt $peak_one ] && peak_one=$one
  sleep 0.5
done
echo "$((peak_sum / 1024)) $((peak_one / 1024))"
