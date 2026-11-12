#!/bin/sh
# merge.sh TAG: add the places runs/TAG/*.miss found (and the addresses
# runs/TAG/*.log stopped at, without the interpreter) to leaders.txt.
. "$(dirname "$0")/common.sh"
L=$LIFT_DATA/leaders.txt
before=$(wc -l < "$L")
{
    cat "$LIFT_DATA/runs/$1"/*.miss 2>/dev/null
    grep -ho "Unlifted at [0-9a-f]*:[0-9a-f]*" "$LIFT_DATA/runs/$1"/*.log 2>/dev/null | sed 's/Unlifted at //'
} | awk '{print $1}' > "$L.new"
awk 'NR==FNR {seen[$1]=1; next} !($1 in seen) {seen[$1]=1; print $1}' "$L" "$L.new" >> "$L"
rm -f "$L.new"
echo "leaders.txt: $before -> $(wc -l < "$L")"
