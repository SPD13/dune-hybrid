#!/bin/sh
# suite.sh record|check [MODE...]: the save-based regression suite.
#
# Cases: the start of the game (seed 12, English and German) and the test
# saves (Day 1 to 23, see SAVES below), each played 300 s with the
# monkey's input, with the machine state hashed every 10 s (STATE_HASH=1)
# and a screenshot every 10 s. Near the end each game writes its state as
# a save (event `save:2`, the game's own serialisation): the save written
# is compared too, byte for byte (the check that will still apply once a
# subsystem's state lives in Rust instead of the 8086 memory).
#
#   record        plain emulation: the reference ($LIFT_DATA/suite/ref)
#   check [ARGS]  the same games with the runner's extra ARGS (default
#                 `--rec on`), compared with the reference: per case, the
#                 first state whose memory or screen differs.
#
# Compared: the whole machine state (CPU, instruction count, memory), the
# screens and the save written; and the subsystems kept in Rust (the clock,
# the troop and location tables) must have seen no write of their memory
# they did not foresee (`clock: 0 sync-ins`, `troops: ...`, `locations: ...`, `text box: ...`, `dialogue text: ...`, `conversation: ...`).
#
# The saves are game-derived and stay outside git (SAVES, default
# ../dune-hybrid/tests-data/saves).
. "$(dirname "$0")/common.sh"
SAVES=${SAVES:-$REPO/../dune-hybrid/tests-data/saves}
S=$LIFT_DATA/suite; mkdir -p $S/ref $S/run
SECS=${SECS:-300}
cd "$REPO"

# name seed lang save
CASES="start12 12 ENG -
start21 21 GER -
day1 1 ENG $SAVES/openrakis/DateTime/Day1At900AndDay1At10h30/DUNE37S2.SAV
day10 10 ENG $SAVES/openrakis/Dialogues/BeforeAfterStillgarSaysHeDidNotKnowAboutErgsunHarg/DUNE37S1.SAV
day13 13 ENG $SAVES/openrakis/DuneTools/DUNE37S1.SAV
day23 23 ENG $SAVES/day23-DUNE37S1.SAV
day23g 31 GER $SAVES/day23-DUNE37S1.SAV"

play() { # out-dir name seed lang save extra...
    out=$1; name=$2; seed=$3; lang=$4; save=$5; shift 5
    # A fresh folder: the game writes and reads its saves there, and a save
    # the monkey made in an earlier run would change this one.
    rm -rf "$out/$name"
    ev="61.5:key:1:1,61.6:key:1:0"; extra=""
    if [ "$save" != "-" ]; then ev="$ev,68:load:1"; extra="--save $save"; fi
    ev="$ev,$((SECS - 5)).5:save:2"
    STATE_HASH=1 $RUN_HAND --dir "$GAME" --cmd "ADP220 SBP2227 $lang" --events "$ev" $extra \
        --seconds $SECS --monkey $seed:66:0.6 --shot-every 10 --out $out/$name "$@" > $out/$name.log 2>&1
    grep '^state' $out/$name.log | awk '{print $2, $5, $6}' > $out/$name.st
    for p in $out/$name/shot-*.png; do
        printf '%s %s\n' "$(basename $p .png | sed 's/shot-//')" "$(sha1sum < $p | cut -c1-16)"
    done > $out/$name.shots
    f=$out/$name/saves/DUNE37S2.SAV
    if [ -f $f ]; then sha1sum < $f | cut -c1-16; else echo none; fi > $out/$name.sav
}

mode=$1; shift
[ "$mode" = record ] || [ "$mode" = check ] || { echo "usage: suite.sh record|check [ARGS]"; exit 2; }
[ "$mode" = check ] && [ $# -eq 0 ] && set -- --rec on
echo "$CASES" | while read name seed lang save; do
    if [ "$save" != "-" ] && [ ! -f "$save" ]; then echo "$name: no save $save, skipped"; continue; fi
    if [ "$mode" = record ]; then
        play $S/ref $name $seed $lang $save
        echo "$name: recorded $(wc -l < $S/ref/$name.st) states"
    else
        play $S/run $name $seed $lang $save "$@"
        dm=$(diff $S/ref/$name.st $S/run/$name.st | grep '^<' | head -1)
        ds=$(diff $S/ref/$name.shots $S/run/$name.shots | grep '^<' | head -1)
        dv=$(diff $S/ref/$name.sav $S/run/$name.sav | grep '^<' | head -1)
        n=$(wc -l < $S/ref/$name.st)
        # The clock kept in Rust (see dune-rec's hand/clock.rs): any
        # unforeseen write of its words fails the case.
        ck=$(grep '^clock:' $S/run/$name.log | sed 's/^clock: //')
        case "$ck" in "0 sync-ins"*) dc="" ;; *) dc="clock [$ck]" ;; esac
        for t in troops locations "text box" "dialogue text" conversation "map view" "map selection" flight "typed message" "screen buffers"; do
            tr=$(grep "^$t:" $S/run/$name.log | sed "s/^$t: //")
            case "$tr" in "0 sync-ins"*) ;; *) dc="$dc $t [$tr]" ;; esac
        done
        if [ -z "$dm$ds$dv$dc" ]; then echo "$name: $n states, the save, the clock and the tables equal ($(cat $S/run/$name.sav))"
        else echo "$name: DIFFERS state [$dm] screen [$ds] save [$dv] $dc"; fi
    fi
done
