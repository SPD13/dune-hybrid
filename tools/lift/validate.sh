#!/bin/sh
# validate.sh TAG: shadow + call-each on seeds 12 and 21, the long-call and
# phase-ladder runs, scripted runs on the day13 save (a troop moved, a
# flight, its videos, Chani asked to stay) and the day23 save (a flight
# from the palace), and --rec on screenshots against ref/off (seed 12), one game
# at a time. Expect only differences marked as the harness's.
. "$(dirname "$0")/common.sh"
S=$LIFT_DATA/validate; mkdir -p $S
T=$1
cd "$REPO"
B=$RUN_HAND
run() { $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0" "$@"; }
# STEPS="d e ..." runs only those runs (a..j, on), keeping the others'
# files, then the summary; STEPS=report only the summary.
want() { [ -z "$STEPS" ] || case " $STEPS " in *" $1 "*) return 0;; *) return 1;; esac; }
[ -n "$STEPS" ] || rm -rf $S/v-$T-*
want a && run --seconds 600 --monkey 12:66:0.6 --rec shadow --rec-report $S/v-$T-a.tsv --call-each 4 --out $S/v-$T-a > $S/v-$T-a.log 2>&1
want b && run --seconds 600 --monkey 21:66:0.6 --rec shadow --rec-report $S/v-$T-b.tsv --call-each 4 --out $S/v-$T-b > $S/v-$T-b.log 2>&1
LONG=${LONG:-c102,2795,281c,b1ee,2806,28a1,2c92,780a,0f95,0d8e,2566,6a33,6a35,6a89,c8fb,afb5,49d9,b427,e675,0e66,0e6c,0e77,1566,37f4,4795,491c,4abe,5692}
want c && run --seconds 120 --monkey 12:66:0.6 --rec shadow --rec-report $S/v-$T-c.tsv --call-each 6 --call-only $LONG --call-ms 3000 --out $S/v-$T-c > $S/v-$T-c.log 2>&1
LATE=${LATE:-4795,5692,b1ee,2566,c8fb,afb5,0e66,0e6c,0e77,1566,37f4,491c,4abe,488a}
want d && run --seconds 380 --monkey 12:66:0.6 --phase-ladder 70:10 --rec shadow --rec-report $S/v-$T-d.tsv --call-each 6 --call-only $LATE --call-ms 2400 --out $S/v-$T-d > $S/v-$T-d.log 2>&1
want on && run --seconds 300 --monkey 12:66:0.6 --rec on --shot-every 30 --out $S/v-$T-on > $S/v-$T-on.log 2>&1
# A troop moved on the map (no monkey): the day13 save, SEE DUNE MAP, a
# troop, MOVE TROOP, a sietch on the small map: `map_click_pick` (81ECh)
# and `move_troop_to` (82DAh), which no monkey run reaches. The save is
# game-derived and stays outside git (SAVES, as suite.sh).
SAVES=${SAVES:-$REPO/../dune-hybrid/tests-data/saves}
click() { echo "$1:mouse:$2:$3:0,$1.5:mouse:$2:$3:1,$1.7:mouse:$2:$3:0"; }
MOVE="68:load:1,$(click 74 130 160),$(click 86 76 92),$(click 92 130 184),$(click 98 177 81)"
# A flight from the same save: SEE DUNE MAP, TAKE AN ORNITHOPTER, a
# sietch on the cockpit's map, SKIP TO DESTINATION; the flight room's
# ports called each (`draw_flight_room` 37F4h, `redraw_flight_room`
# 4ABEh, ...), which the monkey runs never reach.
FLY="68:load:1,$(click 74 130 160),$(click 86 130 184),$(click 96 190 89),$(click 158 130 161)"
FLY_ONLY=37f4,4abe,9f8b,3551,a244,a248,0e66,0e6c,0e77,488a
DAY13=$SAVES/openrakis/DuneTools/DUNE37S1.SAV
if [ -f $DAY13 ]; then
  want e && $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,$MOVE" \
    --save $DAY13 --seconds 110 --rec shadow \
    --rec-report $S/v-$T-e.tsv --out $S/v-$T-e > $S/v-$T-e.log 2>&1
  want f && $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,$FLY" \
    --save $DAY13 --seconds 200 --rec shadow --call-each 6 --call-only $FLY_ONLY --call-ms 3000 \
    --rec-report $S/v-$T-f.tsv --out $S/v-$T-f > $S/v-$T-f.log 2>&1
  # The videos called one at a time, 20 s each, long calls checked
  # (REC_BUDGET): `play_comm_device_video` (0E66h) and
  # `play_wormsuit_video` (0E77h), which the 3 s calls above cut short.
  want g && REC_FROM=68 REC_BUDGET=2000000000 REC_EVENTS=33554432 \
  $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,$FLY" \
    --save $DAY13 --seconds 200 --rec shadow --call-each 1 --call-only 0e66,0e77 --call-ms 20000 \
    --rec-report $S/v-$T-g.tsv --out $S/v-$T-g > $S/v-$T-g.log 2>&1
  # Chani asked to stay, then back (STAY HERE twice): a leave and a join
  # request, `say_entry_mask_20` (9F8Bh).
  STAY="68:load:1,$(click 74 130 192),$(click 82 130 168),$(click 96 130 192),$(click 104 130 168)"
  want i && REC_FROM=68 $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,$STAY" \
    --save $DAY13 --seconds 120 --rec shadow \
    --rec-report $S/v-$T-i.tsv --out $S/v-$T-i > $S/v-$T-i.log 2>&1
else
  for x in e f g i; do echo "no save for the scripted runs" > $S/v-$T-$x.log; : > $S/v-$T-$x.tsv; done
fi
# The day23 save: out of the palace, TAKE AN ORNITHOPTER from the room
# (not the map), a sietch, SKIP TO DESTINATION; the landing plays the
# SAL scene (`play_sal_scene`, 488Ah), a long call (REC_BUDGET).
DAY23=$SAVES/day23-DUNE37S1.SAV
if [ -f $DAY23 ]; then
  PALACE="68:load:1,$(click 74 277 190),$(click 80 277 190),$(click 86 277 190),$(click 90 130 168),$(click 98 190 89),$(click 104 130 161)"
  want h && REC_FROM=68 REC_BUDGET=2000000000 REC_EVENTS=33554432 \
  $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,$PALACE" \
    --save $DAY23 --seconds 170 --rec shadow \
    --rec-report $S/v-$T-h.tsv --out $S/v-$T-h > $S/v-$T-h.log 2>&1
  # Calls with chosen inputs (--call-with) for ports no save reaches:
  # 0E6Ch with video 0Dh (as the load does, 0E51h), C8FBh with video 0Dh
  # and the SAL scene's callback C4CDh (48DDh), and `play_chapter_video`
  # (B1EEh, the book's) with a chapter video, 19h, at [243Eh] (B254h
  # sets it from the table at 2426h: 19h..24h). And 13C8h, the scene
  # script's opcode 0, with the script at CS:1321h (the event at 102Fh):
  # SI past the opcode.
  want j && REC_ONLY=0e6c,c8fb,b1ee,13c8 REC_BUDGET=4000000000 REC_EVENTS=50000000 \
  $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,68:load:1" \
    --save $DAY23 --seconds 75 --rec shadow \
    --call-with "0e6c:AX=000d;c8fb:AX=000d:BP=c4cd;b1ee:[243e]=0019;13c8:SI=1322" --call-ms 120000 \
    --rec-report $S/v-$T-j.tsv --out $S/v-$T-j > $S/v-$T-j.log 2>&1
else
  for x in h j; do echo "no save for the scripted runs" > $S/v-$T-$x.log; : > $S/v-$T-$x.tsv; done
fi

for x in a b c d e f g h i j on; do echo "== $x"; grep -E "^rec|^call-each:.*calls|^call-with|^exit" $S/v-$T-$x.log; done
for x in a b c d e f g h i j; do awk -F'\t' 'NR>1 && $4>0' $S/v-$T-$x.tsv; done
python - "$LIFT_DATA/ref/off" "$S/v-$T-on" <<'PY'
import sys,os,filecmp
a,b=sys.argv[1:]
fs=sorted(f for f in os.listdir(a) if f.endswith('.png'))
print('shots:', sum(filecmp.cmp(os.path.join(a,f),os.path.join(b,f),shallow=False) for f in fs if os.path.exists(os.path.join(b,f))),'/',len(fs),'identical')
PY
echo "hand ports (entry name ok diff bailed skipped), summed:"
cat $S/v-$T-a.tsv $S/v-$T-b.tsv $S/v-$T-c.tsv $S/v-$T-d.tsv $S/v-$T-e.tsv $S/v-$T-f.tsv $S/v-$T-g.tsv $S/v-$T-h.tsv $S/v-$T-i.tsv $S/v-$T-j.tsv | grep "\[hand\]" | awk -F'	' '{k=$1" "$2; ok[k]+=$3; d[k]+=$4; b[k]+=$5; r[k]+=$8} END {for (k in ok) print k"	"ok[k]"	"d[k]"	"b[k]"	"r[k]}' | sort > $S/v-$T-hand.txt
awk -F'	' '{n++; if ($3>0) {bad++; print "DIFF", $0}; if ($2+$4==0) un++; if ($2==0 && $5==0) {nr++}} END {print n" hand ports, "bad+0" with differences, "un+0" never checked, "nr+0" never ran their own code to the end"}' $S/v-$T-hand.txt
