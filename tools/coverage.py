#!/usr/bin/env python3
"""Join `dune-run --coverage` files with the DNCDPRG.lst listing.

    python tools/coverage.py --lst ../Cryogenic/doc/DNCDPRG.lst cov1.txt [cov2.txt ...]

Several coverage files are merged (union). The report shows:
- how many of the listing's seg000 instructions ran, overall and per routine
  (a routine starts at a call target or a named label in the listing);
- routines never entered;
- instruction starts the CPU executed that are not instructions in the listing
  (code the listing shows as data, or decoded at a different offset);
- indirect calls and jumps seen, and their targets.

`--routines FILE` writes the per-routine table as TSV, `--uncovered FILE` the
names of routines never entered.
"""

import argparse
import re
import sys
from collections import defaultdict

INSN = re.compile(r"^seg000:([0-9a-f]{4,5})\s{2,}([a-z]\w*)\b(.*)$")
LABEL = re.compile(r"^seg000:([0-9a-f]{4,5})  ([A-Za-z_]\w*):")
CALL = re.compile(r"^\s*call\s+(?:near ptr |far ptr )?([A-Za-z_]\w*)\s*(?:;.*)?$")
DATA = {"db", "dw", "dd"}


def parse_listing(path):
    insns = {}  # offset -> mnemonic text
    labels = {}  # name -> offset
    called = set()
    with open(path, encoding="latin-1") as f:
        for line in f:
            if line.startswith("; Segment: seg001"):
                break
            m = LABEL.match(line)
            if m:
                labels[m.group(2)] = int(m.group(1), 16)
                continue
            m = INSN.match(line)
            if not m:
                continue
            off, mnem, rest = int(m.group(1), 16), m.group(2), m.group(3)
            if mnem in DATA:
                continue
            insns[off] = (mnem + rest.split(";")[0]).strip()
            c = CALL.match(mnem + rest)
            if mnem == "call" and c:
                called.add(c.group(1))
    starts = {labels[n]: n for n in called if n in labels}
    for name, off in labels.items():
        if not name.startswith("loc_"):
            starts.setdefault(off, name)
    starts.setdefault(0, "start")
    return insns, starts


def read_coverage(paths):
    hit = set()
    indirect = defaultdict(int)
    other = defaultdict(int)
    for p in paths:
        with open(p) as f:
            for line in f:
                parts = line.split()
                if not parts or parts[0].startswith("#"):
                    continue
                if parts[0] == "i":
                    hit.add(int(parts[1], 16))
                elif parts[0] == "j":
                    indirect[(parts[1], parts[2])] += int(parts[3])
                elif parts[0] == "o":
                    other[parts[1]] += int(parts[2])
    return hit, indirect, other


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--lst", required=True)
    ap.add_argument("--routines")
    ap.add_argument("--uncovered")
    ap.add_argument("--top", type=int, default=25, help="largest uncovered routines to list")
    ap.add_argument("coverage", nargs="+")
    a = ap.parse_args()

    insns, starts = parse_listing(a.lst)
    hit, indirect, other = read_coverage(a.coverage)
    order = sorted(starts)

    # Assign each instruction to the routine that starts at or before it.
    per = {s: [0, 0] for s in order}
    idx = 0
    for off in sorted(insns):
        while idx + 1 < len(order) and order[idx + 1] <= off:
            idx += 1
        r = per[order[idx]]
        r[0] += 1
        r[1] += off in hit

    total, done = len(insns), sum(1 for o in insns if o in hit)
    entered = [s for s in order if s in hit]
    print(f"listing: {total} seg000 instructions, {len(order)} routines")
    print(f"covered: {done} instructions ({100 * done / total:.1f}%), "
          f"{len(entered)} routines entered ({100 * len(entered) / len(order):.1f}%)")

    foreign = sorted(h for h in hit if h not in insns)
    if foreign:
        print(f"executed but not an instruction in the listing: {len(foreign)} "
              f"(first: {', '.join(f'{o:04x}' for o in foreign[:12])})")

    targets = defaultdict(set)
    for (frm, to), n in indirect.items():
        targets[frm].add(to)
    unlabelled = set()
    for frm, tos in targets.items():
        for to in tos:
            seg, off = to.split(":")
            if seg == "1000" and int(off, 16) not in starts:
                unlabelled.add(to)
    print(f"indirect sites seen: {len(targets)}, distinct targets: "
          f"{len({t for ts in targets.values() for t in ts})}, "
          f"seg000 targets that start no routine in the listing: {len(unlabelled)}")
    if other:
        segs = ", ".join(f"{s} ({n})" for s, n in sorted(other.items(), key=lambda kv: -kv[1])[:8])
        print(f"outside seg000 (instructions): {segs}")

    never = sorted((per[s][0], s) for s in order if s not in hit)
    never.reverse()
    print(f"\nlargest routines never entered (of {len(never)}):")
    for n, s in never[: a.top]:
        print(f"  {s:04x} {starts[s]:32} {n:5} insns")

    if a.routines:
        with open(a.routines, "w") as f:
            f.write("offset\tname\tinsns\tcovered\tentered\n")
            for s in order:
                f.write(f"{s:04x}\t{starts[s]}\t{per[s][0]}\t{per[s][1]}\t{int(s in hit)}\n")
    if a.uncovered:
        with open(a.uncovered, "w") as f:
            for s in order:
                if s not in hit:
                    f.write(f"{s:04x} {starts[s]}\n")


if __name__ == "__main__":
    sys.exit(main())
