#!/bin/sh
# viewcheck.sh: the graphics tools with and without a copy of the owned
# bytes in the memory (docs/readable-port-plan.md, "The graphics tools read
# through the owners"). Day 23, SECS (110) s of the monkey's play, with the
# HD compositor (x2, its shots), the trace, the driver check (GFX_VERIFY)
# and the room capture on: once with the copy written through (OWN_COPY=1),
# once without (CCh at the owned addresses). Everything written must be
# identical. Output in $LIFT_DATA/viewcheck (game-derived: outside git).
. "$(dirname "$0")/common.sh"; cd "$REPO"
O=$LIFT_DATA/viewcheck; rm -rf $O; mkdir -p $O
SAVE=$REPO/../dune-hybrid/tests-data/saves/day23-DUNE37S1.SAV
for mode in copy poison; do
  if [ $mode = copy ]; then export OWN_COPY=1; else unset OWN_COPY; fi
  mkdir -p $O/$mode
  GFX_VERIFY=1 STATE_HASH=1 $RUN_HAND --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,68:load:1" --save $SAVE \
    --seconds ${SECS:-110} --monkey 23:66:0.6 --shot-every 10 --out $O/$mode --rec on --hd 2 --hd-shots \
    --gfx-trace $O/$mode/trace.jsonl --rooms $O/$mode/rooms > $O/$mode.log 2>&1
  echo "$mode: exit $?"
done
grep -v "MIPS\|wall\| at [0-9.]*s\b" $O/copy.log | grep -v "^speed" > $O/a; grep -v "MIPS\|wall\| at [0-9.]*s\b" $O/poison.log | grep -v "^speed" > $O/b
echo "log diff:"; diff $O/a $O/b | head -20
echo "trace: $(cmp $O/copy/trace.jsonl $O/poison/trace.jsonl && echo same) $(wc -l < $O/copy/trace.jsonl) lines"
echo "out files differing:"; (cd $O/copy && find . -type f ! -name trace.jsonl | sort | while read f; do cmp -s "$f" "../poison/$f" || echo "  $f"; done) | head -20
echo "files: $(cd $O/copy && find . -type f | wc -l) vs $(cd $O/poison && find . -type f | wc -l)"
