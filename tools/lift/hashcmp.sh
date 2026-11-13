#!/bin/sh
# hashcmp.sh TAG SEED SECONDS [SAVE] [LANG]: the same game (seed SEED's
# random input, from the start or from SAVE loaded at 68 s) run twice,
# plain emulation and --rec on, with the whole machine state hashed every
# 10 s (STATE_HASH=1), the memory compared.
# Prints the first hash that differs, or that all are equal. Checks what
# the shadow check can't see: start, the main loop, the timer interrupt.
. "$(dirname "$0")/common.sh"
S=$LIFT_DATA/hashcmp; mkdir -p $S
T=$1; SEED=$2; SECS=$3; SAVE=$4; LANG=${5:-ENG}
cd "$REPO"
EV="61.5:key:1:1,61.6:key:1:0"
EXTRA=""
if [ -n "$SAVE" ] && [ "$SAVE" != "-" ]; then EV="$EV,68:load:1"; EXTRA="--save $SAVE"; fi
run() { STATE_HASH=1 $RUN_HAND --dir "$GAME" --cmd "ADP220 SBP2227 $LANG" --events "$EV" $EXTRA \
  --seconds $SECS --monkey $SEED:66:0.6 --shot-every 10 "$@"; }
run --out $S/$T-off > $S/$T-off.log 2>&1
run --rec on --out $S/$T-on > $S/$T-on.log 2>&1
# The memory part of the hash (the Rust mode's instruction count can
# drift by a few instructions: docs/readable-port-plan.md, first task).
grep '^state' $S/$T-off.log | awk '{split($6,h,"/"); print $2, h[2]}' > $S/$T-off.st
grep '^state' $S/$T-on.log | awk '{split($6,h,"/"); print $2, h[2]}' > $S/$T-on.st
n=$(wc -l < $S/$T-off.st)
d=$(diff $S/$T-off.st $S/$T-on.st | grep '^<' | head -1)
if [ -z "$d" ] && [ "$n" -gt 0 ]; then echo "$T: $n states equal"; else echo "$T: differs from $d (of $n)"; fi
grep -E "^rec interpreted:" $S/$T-on.log
