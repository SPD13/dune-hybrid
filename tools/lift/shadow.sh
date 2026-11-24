#!/bin/bash
# shadow.sh [ENTRIES] [SECS]: shadow-check hand ports in the suite's seven
# games (REC_SAMPLES=200 each), one game at a time. ENTRIES: hex entries,
# comma separated; default every port in the table (hand/mod.rs). Prints
# each game's summary and diff lines, then per entry the calls checked
# (ok, diff, bailed). Sum the "ok" and "diff" columns for the batch:
#   awk '/ ok,/ {ok+=$2; d+=$4} END {print ok, "ok", d, "diff"}'
. "$(dirname "$0")/common.sh"
SAVES=${SAVES:-$REPO/../dune-hybrid/tests-data/saves}
T=$LIFT_DATA/shadow
SECS=${2:-300}
ENTRIES=${1:-$(cd "$REPO" && python -c "
import re;t=open('crates/dune-rec/src/hand/mod.rs',encoding='utf-8').read();print(','.join(sorted(set(re.findall(r'\(0x([0-9a-f]+), \w+::\w+,',t)))))")}
[ -n "$ENTRIES" ] || { echo "shadow.sh: no entries"; exit 1; }
mkdir -p $T
# GAMES="start12 day1 ...": only those games (their TSVs replaced, the
# others' kept), for running the check in parts.
while read name seed lang save; do
    [ -z "$GAMES" ] || case " $GAMES " in *" $name "*) ;; *) continue ;; esac
    ev="61.5:key:1:1,61.6:key:1:0"; extra=""
    if [ "$save" != "-" ]; then ev="$ev,68:load:1"; extra="--save $save"; fi
    rm -rf $T/$name
    REC_ONLY=$ENTRIES REC_SAMPLES=200 NO_PNG=1 $RUN_HAND --dir "$GAME" --cmd "ADP220 SBP2227 $lang" --events "$ev" $extra \
        --seconds $SECS --monkey $seed:66:0.6 --shot-every 100 --out $T/$name --rec shadow \
        --rec-report $T/$name.tsv > $T/$name.log 2>&1
    echo "$name: $(grep '^rec shadow:' $T/$name.log | cut -c13-)"
    grep '^rec diff:' $T/$name.log | cut -c1-200
done <<EOF
start12 12 ENG -
start21 21 GER -
day1 1 ENG $SAVES/openrakis/DateTime/Day1At900AndDay1At10h30/DUNE37S2.SAV
day10 10 ENG $SAVES/openrakis/Dialogues/BeforeAfterStillgarSaysHeDidNotKnowAboutErgsunHarg/DUNE37S1.SAV
day13 13 ENG $SAVES/openrakis/DuneTools/DUNE37S1.SAV
day23 23 ENG $SAVES/day23-DUNE37S1.SAV
day23g 31 GER $SAVES/day23-DUNE37S1.SAV
EOF
for e in ${ENTRIES//,/ }; do
    awk -F'\t' -v e=$e '$1 == e {ok += $3; d += $4; b += $5} END {printf "%s: %d ok, %d diff, %d bailed\n", e, ok, d, b}' $T/*.tsv
done
