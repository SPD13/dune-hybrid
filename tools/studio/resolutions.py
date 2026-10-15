"""Box resolutions: which review boxes have been acted on.

    python resolutions.py WORK ITEM_ID --boxes 1,3|all --version VERSION_ID --note "what was done"

Whoever acts on a box (Claude, or a person running the tools) records it here, in
WORK/review/resolutions.json, never in review.json: the studio page holds review.json
in memory and saves it whole, so a change written there while the page is open would
be lost. The server hands both files to the page.

A box's state, the same rule as the page's (index.html, boxState):
- the user's own action on the box (review.json: "state": "resolved" or "open", with
  "state_at") wins when it is newer than the recorded resolution;
- else a recorded resolution makes it "check": resolved, waiting for the user to confirm;
- else it is "open".
Boxes are numbered from 1, in the order the studio shows them.
"""
import argparse
import json
import os
import sys
import tempfile
import time


def path_of(work):
    return os.path.join(work, "review", "resolutions.json")


def load(work):
    p = path_of(work)
    return json.load(open(p, encoding="utf-8")) if os.path.exists(p) else {"assets": {}}


def box_state(resolutions, item_id, box):
    """(state, version, note) of a review box: state is open, check or resolved."""
    rec = resolutions.get("assets", {}).get(item_id, {}).get("boxes", {}).get(box.get("id"))
    if box.get("state") and (not rec or box.get("state_at", "") >= rec.get("at", "")):
        return box["state"], box.get("resolved_in") or (rec or {}).get("version"), box.get("resolved_note") or (rec or {}).get("note", "")
    if rec:
        return "check", rec.get("version"), rec.get("note", "")
    return "open", None, ""


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    ap = argparse.ArgumentParser(description="Record that review boxes were acted on.")
    ap.add_argument("work")
    ap.add_argument("item", help="item id, e.g. room/859cf422c2ae871b")
    ap.add_argument("--boxes", required=True, help="box numbers (1, 2, …) or all")
    ap.add_argument("--version", required=True, help="the version that resolves them")
    ap.add_argument("--note", default="", help="what was done")
    ap.add_argument("--undo", action="store_true", help="remove the recorded resolution")
    a = ap.parse_args()
    review = json.load(open(os.path.join(a.work, "review", "review.json"), encoding="utf-8"))["assets"]
    boxes = review.get(a.item, {}).get("boxes", [])
    if not boxes:
        sys.exit(f"{a.item}: no boxes")
    nums = range(1, len(boxes) + 1) if a.boxes == "all" else [int(n) for n in a.boxes.split(",")]
    res = load(a.work)
    entry = res.setdefault("assets", {}).setdefault(a.item, {}).setdefault("boxes", {})
    at = time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime())
    for n in nums:
        b = boxes[n - 1]
        if a.undo:
            entry.pop(b["id"], None)
        else:
            entry[b["id"]] = {"version": a.version, "note": a.note, "at": at}
        print(f"box {n}: {'unresolved' if a.undo else 'resolved in ' + a.version} ({b.get('text', '')[:60]})")
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path_of(a.work)), suffix=".tmp")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(res, f, indent=2, ensure_ascii=False)
    os.replace(tmp, path_of(a.work))


if __name__ == "__main__":
    main()
