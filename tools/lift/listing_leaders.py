"""Code pointers in seg000's own data, from DNCDPRG.lst: `dw` values that
point at a code label (jump tables the coverage files may have seen only in
part), and `dd 0000XXXXh` far pointers into seg000 (the interrupt handlers
the program installs, segment filled in at run time); and the labels with
no cross-reference (reached only through a computed address). Prints them as
`1000:XXXX` block starts for dune-lift --leaders."""
import re, sys

labels, dws, dds, lone = set(), [], set(), set()
for l in open(sys.argv[1], encoding="latin-1"):
    m = re.match(r"seg000:([0-9a-f]{4})\s+(loc|sub)_0([0-9a-f]{4})", l)
    if m:
        labels.add(int(m.group(1), 16))
        if "XREF" not in l:
            lone.add(int(m.group(1), 16))
    m = re.match(r"seg000:([0-9a-f]{4})\s+dw\s+([0-9a-f]+)h", l)
    if m:
        dws.append(int(m.group(2), 16))
    m = re.match(r"seg000:([0-9a-f]{4})\s+dd\s+0000([0-9a-f]{4})h", l)
    if m:
        dds.add(int(m.group(2), 16))
for t in sorted(set(t for t in dws if t in labels) | {t for t in dds if 0x100 <= t < 0xF4B0} | lone):
    print(f"1000:{t:04x}")
