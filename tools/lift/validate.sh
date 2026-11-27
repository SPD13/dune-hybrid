#!/bin/sh
# validate.sh TAG: shadow + call-each on seeds 12 and 21, the long-call and
# phase-ladder runs, scripted runs on the day13 save (a troop moved, a
# flight, its videos, Chani asked to stay) and the day23 save (a flight
# from the palace), and --rec on screenshots against ref/off (seed 12), one game
# at a time. Expect 0 differences; calls of the harness that ran other code
# than the lifted (they wrote into code) are counted apart.
. "$(dirname "$0")/common.sh"
S=$LIFT_DATA/validate; mkdir -p $S
T=$1
cd "$REPO"
B=$RUN_HAND
run() { $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0" "$@"; }
# STEPS="d e ..." runs only those runs (a..k, on), keeping the others'
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
  # The rules' paths no game reaches (docs/readable-port-plan.md, "The
  # rules' paths"): calls with chosen inputs from day 23's state. Troop 0
  # (8AAh, at its job since day 12, today 22) at its place 3A0h with a
  # raid ending on day 20 (over) or 23 (on); its motivation 110 (capped),
  # its job 8 (irrigation) or 6 at the player's place, the phase 65h (the
  # late penalty and floor); unarmed (its rate quartered); the day's
  # income from a stock of FFFFh (negative) or 0 (risen); the income
  # raised; a wind-trap's place full (2A4h, water FAh); a new day ([1174h]
  # 0); and the growth with the seed 2000h (first cell 1000h) over two
  # cells of kind 10h at the map's 6613:1000h (the map's segment in these
  # runs). The Harkonnens' attacks: the pick with the attack day set
  # after or before today, outside or in phase 5Ch, the player at the
  # palace; the takeover late, before, too soon or waited after [1154h],
  # on a quiet day, its draw both ways (the skip flag [11BCh] cleared);
  # a raid on 3A0h with both garrison bounds. The smugglers' trade, with
  # smuggler 0 (10D8h) current ([10B4h]) and talking ([47C4h] 0Dh): a
  # first and a later visit (DI at its own record: its byte 0 the key),
  # a restock, an offer with a kind in stock and with none, a purchase
  # (a new debt) and an answer to another talker, refusals and haggles
  # with several seeds of `rand_masked` ([D824h]) and patience 5 or 0,
  # and the talk's end each way ([9Fh], [476Dh]). The troops' movement,
  # with troop 0 at 3A0h: arrivals as a spy (a place newly found, or
  # known), meeting the place's troop (it goes on into 7429h, past the
  # call's limit) or not, an attacker not finding it, a search for
  # equipment at its last place or going on (none spare or some);
  # setting out while travelling or not, an attacker for a near place
  # held or not, a far one; a job given at 7C8h (bulb growing) and job
  # 2; steps straight down and along y with the seed's bit set or
  # clear, fast; the tick with troop 0 held. The meeting (7429h) at
  # 3A0h as a sietch away from the player and as a place raided at it.
  # A chain of troops 1 and 2 at 3A0h: troop 1 merged in the tick; an
  # attacker leaving where a Harkonnen troop stays (abandoned); both
  # idle and unmotivated on a new day (the idle report). The job
  # handlers with troop 1 at 3A0h: mining off or in its slot, held, at
  # a prospected place (spice 2 of worth 4; 10h of worth 1: emptied);
  # a spy's times and draws; bulbs grown; a wind-trap built;
  # irrigation starting a place, growing, drying up; prospecting under
  # way and found, setting out or not; training's exercise, countdown
  # and new drill. The time step at slot 3 with a demand going: the next
  # demand (each sign of the ratio, the messages full), the reminders, one
  # too late, a call waiting, the days counted down. The phase 30h check
  # (Harkonnens at the place or not); character 7 ending the attack
  # (phase 60h). Phase changes (set_game_phase to several phases, the
  # phase change's step: the condition scripts). The runner's `rule paths:` line counts them.
  RULES="6e20:SI=08aa:DI=03a0:[46de]=0001:[03aa]=145c;6e20:SI=08aa:DI=03a0:[46de]=0001:[03aa]=175c;6efd:SI=08aa:[08be]=6e0c;6efd:SI=08aa:[08ac]=0801;6efd:SI=08aa:[08ac]=0601:[114e]=03a0;6efd:SI=08aa:[002a]=0065;708a:SI=08aa:DI=03a0:[08c3]=be00;1c46:[1170]=ffff;1c46:[1170]=0000:[00ae]=0000;c02e:[00a6]=0000;63f0:[46de]=0001:[02be]=fa00;1b23:[46dd]=0001:[1174]=0000;65b6:[1000:65b4]=2000:[6613:1000]=1010;1e43:[1156]=ffff;1e43:[1156]=0000;1e43:[1156]=0000:[002a]=005c:[114e]=07c8;1e43:[1156]=0000:[002a]=005c;1dd7:[002a]=003c:[0000]=0001;1dd7:[1154]=ffff;1dd7:[1154]=0160;1dd7:[1154]=0000:[0002]=0172;1dd7:[1154]=0000:[11bb]=0000:[0000]=8000;1dd7:[1154]=0000:[11bb]=0000:[0000]=0001;7443:SI=08aa:DI=03a0:[0000]=0004;7443:SI=08aa:DI=03a0:[0000]=0001;2318:DI=10d8:[10da]=0000;2318:DI=10d8:[10da]=0808;1c46:[1170]=0000:[10da]=0008:[10dc]=0000:[10e1]=8080;2388:[10b4]=10d8:[47c4]=000d:[11f1]=00e8:[1141]=0003:[10dc]=0100:[10e1]=8c8c;2388:[10b4]=10d8:[47c4]=000d:[11f1]=00e8:[1141]=0003:[10dc]=0000:[10de]=0000;241a:[10b4]=10d8:[47c4]=000d:[11f1]=00e9:[10dc]=0100:[10e6]=0000:[009d]=0018;241a:[47c4]=0003;2431:[10b4]=10d8:[47c4]=000d:[d824]=0000;2431:[10b4]=10d8:[47c4]=000d:[d824]=0001;2431:[10b4]=10d8:[47c4]=000d:[d824]=1234;2431:[10b4]=10d8:[47c4]=000d:[d824]=5678;2453:[10b4]=10d8:[47c4]=000d:[d824]=0000:[10d9]=0005:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=0001:[10d9]=0005:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=1234:[10d9]=0000:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=5678:[10d9]=0005:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=9abc:[10d9]=0005:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=0001:[10d9]=0000:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=5678:[10d9]=0000:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=9abc:[10d9]=0000:[001a]=0000;2453:[10b4]=10d8:[47c4]=000d:[d824]=4321:[10d9]=0000:[001a]=0000;24ee:[009e]=0100:[476c]=0000:[001a]=0002;24ee:[009e]=0100:[476c]=0100:[10b4]=10d8:[10e6]=0040;24ee:[009e]=0200:[476c]=0100:[10b4]=10d8;24ee:[009e]=0300:[476c]=0100:[10b4]=10d8;8357:SI=08aa:[08ae]=03a0:[08ac]=0501:[03aa]=1482;8357:SI=08aa:[08ae]=03a0:[08ac]=0501:[03aa]=1400:[03a8]=0028;8357:SI=08aa:[08ae]=03a0:[08ac]=0101:[03aa]=1400:[03a8]=0028;8357:SI=08aa:[08ae]=03a0:[08ac]=0601:[03aa]=1400:[03a8]=0010;8357:SI=08aa:[08ae]=03a0:[08ac]=0301:[03aa]=1400:[03a8]=0010:[08b6]=03a0;8357:SI=08aa:[08ae]=03a0:[08ac]=0301:[03aa]=1400:[03a8]=0010:[08b6]=0100:[08b8]=0402:[03b4]=0000:[03b6]=0000;8357:SI=08aa:[08ae]=03a0:[08ac]=0301:[03aa]=1400:[03a8]=0010:[08b6]=0100:[08b8]=0402:[03b4]=0505:[03b6]=0505;84a6:SI=08aa:DI=0100:[08ac]=4301;84a6:SI=08aa:DI=0100:[08ac]=4101;84a6:SI=08aa:DI=0100:[08ac]=0101:[08ae]=03a0;84a6:SI=08aa:DI=0100:[08ac]=0601:[08ae]=03a0:[03aa]=1402:[0108]=0010:[010a]=0002;84a6:SI=08aa:DI=0100:[08ac]=0601:[08ae]=03a0:[03aa]=1402:[0108]=0010:[010a]=0000;84a6:SI=08aa:DI=0100:[08ac]=0601:[08ae]=03a0:[03aa]=1402:[0108]=0030:[010a]=0000;84a6:SI=08aa:DI=0100:[08ac]=0601:[08ae]=03a0:[03aa]=1402:[0108]=0030:[010a]=0000:[08c9]=03a0:[08d5]=0080;6ad4:SI=08aa:CX=0008:[08ae]=07c8:[07e2]=0000;6ad4:SI=08aa:CX=0002:[08ae]=03a0;8308:SI=08aa:[08ae]=03a0:[03a2]=0100:[03a4]=0050:[08b0]=0100:[08b2]=0010;8308:SI=08aa:[08ae]=03a0:[03a2]=0100:[03a4]=0050:[08b0]=0110:[08b2]=0010:[0000]=8000;8308:SI=08aa:[08ae]=03a0:[03a2]=0100:[03a4]=0050:[08b0]=0110:[08b2]=0010:[0000]=0001:[08c3]=be40;6c6f:[08bc]=0400;7429:SI=08aa:DI=03a0:[03a8]=0010:[03aa]=1402;7429:SI=08aa:DI=03a0:[114e]=03a0:[03a8]=0030;6c6f:[08aa]=0201:[08ae]=03a0:[08c5]=0002:[08c9]=03a0:[08ac]=0401:[08ba]=0000:[08bc]=0000:[08c3]=0500:[08c7]=0402:[08de]=0500:[03a8]=0128;84a6:SI=08aa:DI=0100:[08aa]=0201:[08ae]=03a0:[08c5]=0002:[08c9]=03a0:[08ac]=0601:[08c7]=0402:[08d5]=0080:[03aa]=1402:[03a8]=0128:[0108]=0030:[010a]=0000;6e20:SI=08aa:DI=03a0:[46de]=0001:[114e]=07c8:[08aa]=0201:[08ae]=03a0:[08c5]=0002:[08c9]=03a0:[08ac]=0001:[08bc]=0080:[08bf]=0010:[08c7]=0002:[08d7]=0000:[08da]=0010:[03a8]=0110:[03aa]=1400;6fe5:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0001:[08ba]=0200:[0002]=0012;6fe5:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0001:[08ba]=0200:[0002]=0011:[08bc]=0000:[03b1]=0204:[03aa]=1440;6fe5:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0001:[08ba]=0000:[08bc]=0030;6fe5:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0001:[08ba]=0000:[08bc]=0000:[03b1]=0204:[03aa]=1440;6fe5:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0001:[08ba]=0000:[08bc]=0000:[03b1]=1001:[03aa]=1440:[08bf]=f064:[08c3]=ff80;72b0:SI=08aa:DI=03a0:[08ae]=03a0:[08ba]=0000:[08c1]=0010:[0002]=0100:[08b4]=00d0:[08b6]=0005:[03a8]=0010:[d826]=0000;72b0:SI=08aa:DI=03a0:[08ae]=03a0:[08ba]=0000:[08c1]=0010:[0002]=0100:[08b4]=00d0:[08b6]=0005:[03a8]=0010:[d826]=1234;72b0:SI=08aa:DI=03a0:[08ae]=03a0:[08ba]=0000:[08c1]=0010:[0002]=0100:[08b4]=00e8:[08b6]=0005:[03a8]=0010;72b0:SI=08aa:DI=03a0:[08ae]=03a0:[08ba]=0000:[08c1]=0010:[0002]=0100:[08b4]=00f8;767d:SI=08aa:DI=03a0:[08ae]=03a0:[03ba]=0000:[00ec]=00ff;7711:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[03ba]=ff00;7693:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0801:[08bc]=0000:[08c3]=be02:[03aa]=1420:[03ba]=2000;7693:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0801:[08bc]=0000:[08c3]=be02:[03aa]=1421:[03ae]=ff10:[03ba]=2000;7693:SI=08aa:DI=03a0:[08ae]=03a0:[08ac]=0801:[08bc]=0000:[08c3]=be02:[03aa]=1421:[03ae]=ff10:[03ba]=0500;70cc:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[08b6]=0000:[03b0]=0400:[08bf]=0010:[0002]=0100:[08b4]=00ff;70cc:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[08b6]=0000:[03b0]=0400:[08bf]=0010:[0002]=0100:[08b4]=00f0:[11d3]=0000;70cc:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[08b6]=0000:[03b0]=0400:[08bf]=0010:[0002]=0100:[08b4]=00f0:[11d3]=03a0;71ef:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1404:[08b8]=0000;71ef:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1404:[08b8]=ff00;71ef:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[08b6]=0005;71ef:SI=08aa:DI=03a0:[08ae]=03a0:[03aa]=1400:[08b6]=0000;6c6f:[08bc]=0000:[08c3]=0500:[08ac]=0401:[08ae]=03a0:[08c7]=0400:[08c9]=03a0;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=8000:[118d]=0016;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=8040:[118d]=0016:[00c8]=000a;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=80c0:[118d]=0016:[00c2]=0000;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=9040:[118d]=0015;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=9000:[118d]=0013;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=9090:[118d]=0014;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=9040:[118d]=0010;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=8040:[118d]=0010:[11bb]=0001;1b23:[46dd]=0001:[0002]=0163:[1174]=0163:[00be]=8040:[118d]=0010;6c46:[002a]=002e:[473c]=03a0:[0010]=0000;6c46:[002a]=002e:[473c]=03a0:[0010]=0000:[03a8]=0110:[08aa]=0001:[08ae]=03a0:[08ac]=0401:[08ba]=0080;1d9f:[002a]=005d:[0012]=0000:[1056]=0007:[104a]=0280:[00f9]=00f8:[0124]=0128:[08aa]=0001:[08ae]=011c:[08bc]=0400;121f:AX=0030:[002a]=002e;121f:AX=0018:[002a]=0014;121f:AX=0060:[002a]=005d;121f:AX=0034:[002a]=0030;121f:AX=0040:[002a]=003c;121f:AX=0008:[002a]=0004;b17a;b17a:[002a]=0010;b17a:[002a]=0040"
  # NATIVE_CHECK: the native rules (`hand/world.rs`) checked against
  # the ports they cover at each call (the `native:` lines).
  want k && NATIVE_CHECK=1 REC_ONLY=121f,b17a,6c46,1d9f,6e20,6efd,708a,1c46,c02e,63f0,65b6,1b23,642e,1e43,1dd7,7443,2318,235f,2388,23d5,23e6,241a,2431,2453,24ee,6c6f,8308,8604,8357,84a6,6ad4,7429,6d19,6fe5,72b0,767d,7711,7693,70cc,71ef REC_BUDGET=4000000000 REC_EVENTS=50000000   $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0,68:load:1"     --save $DAY23 --seconds 75 --rec shadow --call-with "$RULES" --call-ms 20000     --rec-report $S/v-$T-k.tsv --out $S/v-$T-k > $S/v-$T-k.log 2>&1
else
  for x in h j k; do echo "no save for the scripted runs" > $S/v-$T-$x.log; : > $S/v-$T-$x.tsv; done
fi

for x in a b c d e f g h i j k on; do echo "== $x"; grep -E "^rec|^call-each:.*calls|^call-with|^exit|^native" $S/v-$T-$x.log; done
for x in a b c d e f g h i j k; do awk -F'\t' 'NR>1 && $4>0' $S/v-$T-$x.tsv; done
python - "$LIFT_DATA/ref/off" "$S/v-$T-on" <<'PY'
import sys,os,filecmp
a,b=sys.argv[1:]
fs=sorted(f for f in os.listdir(a) if f.endswith('.png'))
print('shots:', sum(filecmp.cmp(os.path.join(a,f),os.path.join(b,f),shallow=False) for f in fs if os.path.exists(os.path.join(b,f))),'/',len(fs),'identical')
PY
echo "hand ports (entry name ok diff bailed skipped), summed:"
cat $S/v-$T-a.tsv $S/v-$T-b.tsv $S/v-$T-c.tsv $S/v-$T-d.tsv $S/v-$T-e.tsv $S/v-$T-f.tsv $S/v-$T-g.tsv $S/v-$T-h.tsv $S/v-$T-i.tsv $S/v-$T-j.tsv $S/v-$T-k.tsv | grep "\[hand\]" | awk -F'	' '{k=$1" "$2; ok[k]+=$3; d[k]+=$4; b[k]+=$5; r[k]+=$8; f[k]+=$9} END {for (k in ok) print k"	"ok[k]"	"d[k]"	"b[k]"	"r[k]"	"f[k]}' | sort > $S/v-$T-hand.txt
awk -F'	' '{n++; if ($3>0) {bad++; print "DIFF", $0}; if ($2+$4==0) un++; if ($2==0 && $5==0) {nr++}; if ($6>0) fo++} END {print n" hand ports, "bad+0" with differences, "un+0" never checked, "nr+0" never ran their own code to the end, "fo+0" with harness calls running other code than the lifted"}' $S/v-$T-hand.txt
