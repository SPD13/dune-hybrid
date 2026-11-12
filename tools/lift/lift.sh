#!/bin/sh
# Lift seg000 and the three drivers into crates/dune-rec/src/generated*.rs,
# then build the runner with and without the interpreter.
#
# Inputs, all in $LIFT_DATA (see common.sh):
#   cov/*.txt            --coverage files (indirect targets, executed code)
#   drivers/drv*/        driver images dumped with --dump-drivers; drv300 is
#                        the main one, the others are forms rewritten at run time
#   drivers/forms/       more such forms, dumped by soak.sh (REC_DRIVER_FORMS)
#   drivers/vga-rewritten.txt  graphics driver instructions rewritten at run
#                        time that no dump shows rewritten (one hex offset a line)
#   drivers/dcov*.txt    coverage of the driver segments
#   leaders.txt          extra block starts (SEG:OFF [count]), from REC_MISSES
# The hand ports' stretch starts are block starts too; they are read from
# crates/dune-rec/src/hand/ each time, and so are the code pointers in
# seg000's tables (from the listing, listing_leaders.py).
set -e
. "$(dirname "$0")/common.sh"
cd "$REPO"
D=$LIFT_DATA/drivers
CARGO_TARGET_DIR=$HAND_TARGET cargo build --release -p lift
LIFT=$HAND_TARGET/release/dune-lift
HAND=$LIFT_DATA/leaders-hand.txt
grep -rhoE "c\.(run|check)\(0x[0-9a-f]{4}" crates/dune-rec/src/hand/ | sed -E 's/.*0x/1000:/' | sort -u > "$HAND"
DW=$LIFT_DATA/leaders-listing.txt
python tools/lift/listing_leaders.py "$LST" > "$DW"
echo "$(wc -l < "$DW") block starts from the listing (code pointers, unreferenced labels)"
echo "$(wc -l < "$HAND") hand port stretch starts, $(wc -l < "$LIFT_DATA/leaders.txt") extra block starts"

$LIFT --exe "$GAME/DNCDPRG.EXE" --lst "$LST" --leaders "$LIFT_DATA/leaders.txt" --leaders "$HAND" --leaders "$DW" \
    --out crates/dune-rec/src/generated.rs "$LIFT_DATA"/cov/*.txt

# drv KIND SEG SLOTS NAME FORMS [extra args]
drv() {
    kind=$1 seg=$2 slots=$3 name=$4 forms=$5; shift 5
    also=""
    for f in $forms; do also="$also --also $D/$f/$kind.bin"; done
    # Forms the soak games found (REC_DRIVER_FORMS).
    for f in "$D"/forms/"$kind"-*/"$kind".bin; do [ -f "$f" ] && also="$also --also $f"; done
    $LIFT --raw "$D/drv300/$kind.bin" --seg "$seg" --slots "$slots" --name "$name" $also \
        --cov "$D/dcov1.txt" --cov "$D/dcov2.txt" --cov "$D/dcov3.txt" \
        --leaders "$LIFT_DATA/leaders.txt" "$@" --out "crates/dune-rec/src/generated_$name.rs"
}
drv graphics 334b 46 vga "drvE1 drvE10 drv70 drvA drvB drvC" --rewritten "$(paste -sd, "$D/vga-rewritten.txt")"
drv music 5ba1 7 music "drv70 drvA drvB drvC drvE10"
drv sound 5635 8 sound "drv70 drvA drvB drvC drvE10" --entries 03ad,0591

CARGO_TARGET_DIR=$HAND_TARGET cargo build --release -p runner
CARGO_TARGET_DIR=$NOINT_TARGET cargo build --release -p runner --no-default-features
