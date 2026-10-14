"""Background jobs (setup, downloads, generations), run one at a time since
they share the GPU, with progress read from their output:

- `@progress F`: F (0-1) of the current step;
- `@stage TEXT`: what the step is doing (`... (k/n)` also places it within the step);
- `@plan N renders`: the step will run N generations;
- tqdm bars (`12/25 [`, `40%|`): progress within the current generation or download.
"""
import collections
import itertools
import os
import re
import subprocess
import threading
import time

_ids = itertools.count(1)
TQDM_STEPS = re.compile(r"(\d+)/(\d+) \[")
TQDM_PCT = re.compile(r"(\d+)%\|")
STAGE_K = re.compile(r"\((\d+)/(\d+)\)")


class Job:
    def __init__(self, title, steps, env=None, cwd=None, on_done=None):
        self.id = next(_ids)
        self.title = title
        self.steps = steps            # [(label, argv list | callable(job))]
        self.env, self.cwd, self.on_done = env, cwd, on_done
        self.status = "queued"
        self.progress = 0.0
        self.step = ""
        self.message = ""
        self.log = collections.deque(maxlen=300)
        self.created = time.time()
        self.started = self.ended = None
        self.proc = None
        self.cancelled = False
        self._plan = 0
        self._k = 0

    def view(self):
        return {"id": self.id, "title": self.title, "status": self.status, "progress": round(self.progress, 4), "step": self.step,
                "message": self.message, "log": list(self.log)[-40:], "created": self.created, "started": self.started, "ended": self.ended}

    def _line(self, i, n, line):
        line = line.rstrip()
        if not line:
            return
        frac = None
        if line.startswith("@progress "):
            try:
                frac = float(line.split()[1])
            except ValueError:
                pass
        elif line.startswith("@plan "):
            self._plan = int(line.split()[1]) if line.split()[1].isdigit() else 0
        elif line.startswith("@stage "):
            self.message = line[7:]
            m = STAGE_K.search(line)
            if m:
                self._k = int(m.group(1)) - 1
                frac = self._k / max(int(m.group(2)), 1)
        elif line.startswith("@device "):
            self.message = "running on " + line[8:]
        else:
            # Loading bars (weights, pipeline parts) are not progress of the work.
            m = None if line.lstrip().startswith("Loading") else TQDM_STEPS.search(line) or TQDM_PCT.search(line)
            if m:
                inner = int(m.group(1)) / max(int(m.group(2)), 1) if m.re is TQDM_STEPS else int(m.group(1)) / 100
                frac = (self._k + inner) / self._plan if self._plan else inner
            if not line.startswith("@"):
                self.log.append(line[-300:])
        if frac is not None:
            self.progress = max(self.progress, (i + min(max(frac, 0), 1)) / n)

    def run(self):
        self.status, self.started = "running", time.time()
        n = len(self.steps)
        try:
            for i, (label, step) in enumerate(self.steps):
                if self.cancelled:
                    break
                self.step, self.message, self._plan, self._k = label, "", 0, 0
                self.log.append(f"== {label}")
                if callable(step):
                    step(self)
                else:
                    self.log.append("$ " + " ".join(step))
                    self.proc = subprocess.Popen(step, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, cwd=self.cwd,
                                                 env={**os.environ, "PYTHONUNBUFFERED": "1", **(self.env or {})}, bufsize=0)
                    buf = b""
                    while True:
                        ch = self.proc.stdout.read(1)
                        if not ch:
                            break
                        if ch in (b"\n", b"\r"):
                            self._line(i, n, buf.decode("utf-8", "replace"))
                            buf = b""
                        else:
                            buf += ch
                    self._line(i, n, buf.decode("utf-8", "replace"))
                    code = self.proc.wait()
                    if code != 0 and not self.cancelled:
                        raise RuntimeError(f"{label}: exit code {code}")
                self.progress = max(self.progress, (i + 1) / n)
            self.status = "cancelled" if self.cancelled else "done"
        except Exception as e:  # noqa: BLE001 — shown to the user
            self.status, self.message = "failed", str(e)
            self.log.append(f"!! {e}")
        self.ended = time.time()
        if self.on_done:
            try:
                self.on_done(self)
            except Exception as e:  # noqa: BLE001
                self.log.append(f"!! after the job: {e}")

    def cancel(self):
        self.cancelled = True
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()


class Queue:
    def __init__(self):
        self.jobs = []
        self.lock = threading.Lock()
        self.wake = threading.Event()
        threading.Thread(target=self._loop, daemon=True).start()

    def add(self, job):
        with self.lock:
            self.jobs.append(job)
        self.wake.set()
        return job

    def get(self, job_id):
        return next((j for j in self.jobs if j.id == job_id), None)

    def busy(self, title):
        return any(j.title == title and j.status in ("queued", "running") for j in self.jobs)

    def _loop(self):
        while True:
            job = next((j for j in self.jobs if j.status == "queued"), None)
            if job is None:
                self.wake.wait()
                self.wake.clear()
                continue
            if job.cancelled:
                job.status = "cancelled"
                continue
            job.run()
