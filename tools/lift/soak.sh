#!/bin/sh
# Games, one at a time (several at once run the machine out of memory).
#
#   soak.sh collect TAG SPEC...   the build with the interpreter, --rec native:
#                                 writes where Rust had no block to go on from
#                                 (REC_MISSES) and where the interpreter ran
#                                 (REC_INTERP); `merge` adds the first to
#                                 leaders.txt for the next lift. Driver code
#                                 rewritten into forms not lifted goes to
#                                 drivers/forms/ (REC_DRIVER_FORMS).
#   soak.sh check TAG SPEC...     the build without the interpreter: where
#                                 each game stops, if it does.
#
# SPEC: LANG,SEED,SECONDS[,LADDER[,STEP]]  e.g. ENG,12,2400 or GER,5,2400,70:30
#   LADDER: --phase-ladder START:STEP (story phases); STEP: monkey click
#   interval in seconds (default 0.6). SEED 0 = no input at all (the intro).
# Results go to $LIFT_DATA/runs/TAG/.
. "$(dirname "$0")/common.sh"
mode=$1 tag=$2; shift 2
out=$LIFT_DATA/runs/$tag
mkdir -p "$out"
case $mode in
    collect) B=$RUN_HAND ;;
    check) B=$RUN_NOINT ;;
    *) echo "soak.sh collect|check TAG SPEC..."; exit 2 ;;
esac
for spec in "$@"; do
    IFS=, read -r lang seed secs ladder step <<E
$spec
E
    name=$(echo "$spec" | tr ',:' '_-')
    set -- --dir "$GAME" --cmd "ADP220 SBP2227 $lang" --seconds "$secs" --rec native --shot-every 60 --out "$out/$name"
    if [ "$seed" != 0 ]; then
        set -- "$@" --events "61.5:key:1:1,61.6:key:1:0" --monkey "$seed:66:${step:-0.6}"
    fi
    [ -n "$ladder" ] && set -- "$@" --phase-ladder "$ladder"
    if [ $mode = collect ]; then
        REC_DRIVER_FORMS=$LIFT_DATA/drivers/forms REC_MISSES=$out/$name.miss REC_INTERP=$out/$name.interp "$B" "$@" > "$out/$name.log" 2>&1
    else
        "$B" "$@" > "$out/$name.log" 2>&1
    fi
    printf '%-28s %s\n' "$spec" "$(grep '^exit' "$out/$name.log" | tail -1)"
    [ $mode = collect ] && printf '%-28s %s misses, %s interpreted places\n' "" "$(wc -l < "$out/$name.miss" 2>/dev/null)" "$(wc -l < "$out/$name.interp" 2>/dev/null)"
done
exit 0
