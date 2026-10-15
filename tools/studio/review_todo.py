"""What the review asks for: the studio's annotations, for whoever runs the
next generation round (a person or Claude).

    python review_todo.py WORK_DIR [--all] [--json]

Lists the items marked "needs work" or "rejected", and every item with notes
or open boxes, with each box in the original's pixels, its kind and text; then the
approved items. Boxes already acted on (resolutions.py) are listed as waiting for the
user's check; confirmed ones are left out (--all shows them). With --json, the same as JSON.
"""
import argparse
import json
import os
import sys

import resolutions as res_mod


def main():
    sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252

    ap = argparse.ArgumentParser()
    ap.add_argument("work")
    ap.add_argument("--all", action="store_true", help="also list items reviewed without notes or boxes")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()
    items = {i["id"]: i for i in json.load(open(os.path.join(a.work, "review", "items.json")))["items"]}
    path = os.path.join(a.work, "review", "review.json")
    review = json.load(open(path, encoding="utf-8"))["assets"] if os.path.exists(path) else {}
    resolutions = res_mod.load(a.work)
    todo, approved = [], []
    for item_id, r in sorted(review.items(), key=lambda kv: kv[1].get("updated", ""), reverse=True):
        it = items.get(item_id, {})
        entry = {"id": item_id, "title": it.get("title", item_id), "class": it.get("class"), "status": r.get("status"),
                 "preferred": r.get("preferred"), "notes": r.get("notes", "").strip(), "updated": r.get("updated"),
                 "boxes": []}
        for n, b in enumerate(r.get("boxes", []), 1):
            state, version, note = res_mod.box_state(resolutions, item_id, b)
            if state == "resolved" and not a.all:
                continue
            entry["boxes"].append({"n": n, "kind": b["kind"], "text": b.get("text", "").strip(), "px": b.get("px"), "on": b.get("version"),
                                   "state": state, "resolved_in": version, "note": note})
        if r.get("status") == "approved" and not entry["notes"] and not entry["boxes"]:
            approved.append(entry)
        elif r.get("status") in ("needs-work", "rejected") or entry["notes"] or entry["boxes"] or a.all:
            todo.append(entry)
    if a.json:
        print(json.dumps({"todo": todo, "approved": approved}, indent=2, ensure_ascii=False))
        return
    print(f"{len(todo)} items to act on, {len(approved)} approved\n")
    for e in todo:
        print(f"■ {e['title']}  [{e['id']}]  {e['class']} — {e['status']}" + (f", preferred {e['preferred']}" if e["preferred"] else ""))
        if e["notes"]:
            print("  notes: " + e["notes"].replace("\n", "\n         "))
        for b in e["boxes"]:
            done = {"check": f"  [resolved in {b['resolved_in']}, waiting for the user's check: {b['note']}]",
                    "resolved": f"  [resolved in {b['resolved_in']}: {b['note']}]"}.get(b["state"], "")
            print(f"  box {b['n']} ({b['kind']}) at {b['px']} on {b['on']}: {b['text'] or '(no text)'}{done}")
        print()
    if approved:
        print("approved: " + ", ".join(e["title"] for e in approved))


if __name__ == "__main__":
    main()
