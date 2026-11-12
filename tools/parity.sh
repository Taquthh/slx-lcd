#!/usr/bin/env bash
# Compare lcd-core byte streams against the upstream Python Rev C driver.
# Usage: tools/parity.sh /path/to/turing-smart-screen-python
set -euo pipefail
UP="${1:?path to turing-smart-screen-python}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/target/parity"
rm -rf "$OUT"
python3 "$ROOT/tools/dump_frames_py.py" "$UP" "$OUT/py" 2>/dev/null
(cd "$ROOT" && cargo run -q --release --example dump_frames -- "$OUT/rs")
fail=0
for f in "$OUT"/py/*.bin; do
  n=$(basename "$f")
  if cmp -s "$OUT/py/$n" "$OUT/rs/$n"; then echo "OK   $n"; else echo "DIFF $n"; fail=1; fi
done
exit $fail
