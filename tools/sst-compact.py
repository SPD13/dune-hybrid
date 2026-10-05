# Convert SingleStepTests 80286 MOO files into compact JSON (no cycle traces).
import gzip, json, os, sys
from concurrent.futures import ProcessPoolExecutor
import moo2json

SRC = "ss286/v1_real_mode"
DST = "286"
REVOKED = set(open("ss286/revocation_list.txt").read().split())

def one(name):
    data = gzip.open(os.path.join(SRC, name)).read()
    _, tests = moo2json.parse_moo_bytes(data)
    out = []
    for t in tests:
        if t.get("hash") in REVOKED:
            continue
        out.append({
            "name": t["name"],
            "initial": {"regs": t["initial"]["regs"], "ram": t["initial"]["ram"]},
            "final": {"regs": t["final"]["regs"], "ram": t["final"]["ram"]},
            "exception": t.get("exception"),
        })
    dst = os.path.join(DST, name.replace(".MOO.gz", ".json.gz"))
    with gzip.open(dst, "wt") as f:
        json.dump(out, f)
    return name, len(out)

if __name__ == "__main__":
    os.makedirs(DST, exist_ok=True)
    names = sorted(n for n in os.listdir(SRC) if n.endswith(".MOO.gz"))
    total = 0
    with ProcessPoolExecutor() as ex:
        for name, n in ex.map(one, names):
            total += n
    print(len(names), "files", total, "tests")
