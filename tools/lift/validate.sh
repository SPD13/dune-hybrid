#!/bin/sh
# validate.sh TAG: shadow + call-each on seeds 12 and 21, the long-call and
# phase-ladder runs, and --rec on screenshots against ref/off (seed 12),
# one game at a time. Expect only differences marked as the harness's.
. "$(dirname "$0")/common.sh"
S=$LIFT_DATA/validate; mkdir -p $S
T=$1
cd "$REPO"
B=$RUN_HAND
run() { $B --dir "$GAME" --cmd "ADP220 SBP2227 ENG" --events "61.5:key:1:1,61.6:key:1:0" "$@"; }
rm -rf $S/v-$T-*
run --seconds 600 --monkey 12:66:0.6 --rec shadow --rec-report $S/v-$T-a.tsv --call-each 4 --out $S/v-$T-a > $S/v-$T-a.log 2>&1
run --seconds 600 --monkey 21:66:0.6 --rec shadow --rec-report $S/v-$T-b.tsv --call-each 4 --out $S/v-$T-b > $S/v-$T-b.log 2>&1
LONG=${LONG:-c102,2795,281c,b1ee,2806,28a1,2c92,780a,0f95,0d8e,2566,6a33,6a35,6a89,c8fb,afb5,49d9,b427,e675,0e66,0e6c,0e77,1566,37f4,4795,491c,4abe,5692}
run --seconds 120 --monkey 12:66:0.6 --rec shadow --rec-report $S/v-$T-c.tsv --call-each 6 --call-only $LONG --call-ms 3000 --out $S/v-$T-c > $S/v-$T-c.log 2>&1
LATE=${LATE:-4795,5692,b1ee,2566,c8fb,afb5,0e66,0e6c,0e77,1566,37f4,491c,4abe,488a}
run --seconds 380 --monkey 12:66:0.6 --phase-ladder 70:10 --rec shadow --rec-report $S/v-$T-d.tsv --call-each 6 --call-only $LATE --call-ms 2400 --out $S/v-$T-d > $S/v-$T-d.log 2>&1
run --seconds 300 --monkey 12:66:0.6 --rec on --shot-every 30 --out $S/v-$T-on > $S/v-$T-on.log 2>&1

for x in a b c d on; do echo "== $x"; grep -E "^rec|^call-each:.*calls|^exit" $S/v-$T-$x.log; done
for x in a b c d; do awk -F'\t' 'NR>1 && $4>0' $S/v-$T-$x.tsv; done
python - "$LIFT_DATA/ref/off" "$S/v-$T-on" <<'PY'
import sys,os,filecmp
a,b=sys.argv[1:]
fs=sorted(f for f in os.listdir(a) if f.endswith('.png'))
print('shots:', sum(filecmp.cmp(os.path.join(a,f),os.path.join(b,f),shallow=False) for f in fs if os.path.exists(os.path.join(b,f))),'/',len(fs),'identical')
PY
echo "hand ports (entry name ok diff bailed skipped), summed:"
cat $S/v-$T-a.tsv $S/v-$T-b.tsv $S/v-$T-c.tsv $S/v-$T-d.tsv | grep "\[hand\]" | awk -F'	' '{k=$1" "$2; ok[k]+=$3; d[k]+=$4; b[k]+=$5; r[k]+=$8} END {for (k in ok) print k"	"ok[k]"	"d[k]"	"b[k]"	"r[k]}' | sort > $S/v-$T-hand.txt
awk -F'	' '{n++; if ($3>0) {bad++; print "DIFF", $0}; if ($2+$4==0) un++; if ($2==0 && $5==0) {nr++}} END {print n" hand ports, "bad+0" with differences, "un+0" never checked, "nr+0" never ran their own code to the end"}' $S/v-$T-hand.txt
