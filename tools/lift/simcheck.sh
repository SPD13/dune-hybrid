#!/bin/sh
# The simulator's long cross-check: from a save, a day at a time, the
# simulator without the emulator (--sim-only 16 clock steps) writes the
# next day's save; the original loads it and runs its next time steps
# with NATIVE_CHECK and NATIVE_SIM (the rules carrying their own world,
# compared after each step). DAYS days; the next day starts from the
# simulator's save, not the original's.
#   SAVE=FILE DAYS=N sh tools/lift/simcheck.sh
. "$(dirname "$0")/common.sh"
SAVE=${SAVE:-$REPO/../dune-hybrid/tests-data/saves/day23-DUNE37S1.SAV}
DAYS=${DAYS:-5}
SECS=${SECS:-300}
S=$LIFT_DATA/simcheck; mkdir -p $S
cur=$SAVE
d=0
while [ $d -lt $DAYS ]; do
    rm -rf $S/sim-$d $S/run-$d
    $RUN_HAND --dir "$GAME" --save "$cur" --sim-only 16 --out $S/sim-$d | sed "s/^/day $d: /"
    cur=$S/sim-$d/saves/DUNE37S9.SAV
    NATIVE_CHECK=1 NATIVE_SIM=1 $RUN_HAND --dir "$GAME" --cmd "ADP220 SBP2227 ENG" \
        --events "61.5:key:1:1,61.6:key:1:0,68:load:1" --save "$cur" --seconds $SECS \
        --shot-every 50 --out $S/run-$d --rec on > $S/run-$d.log 2>&1
    grep -E "^native sim:|^native: time step|^native not modelled|^save files" $S/run-$d.log | sed "s/^/day $d: /"
    grep -E "^native sim( \(deferred\))? at" $S/run-$d.log | head -3 | cut -c1-300 | sed "s/^/day $d: /"
    d=$((d + 1))
done
