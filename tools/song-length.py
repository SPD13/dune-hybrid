#!/usr/bin/env python3
"""Length of Dune's AdLib (DNADP/HERAD) songs, from the song data alone.

Timing-only model of the driver, from the documented logic in Cryogenic's
Apache-2.0 AdpPlayer (DuneAdpPlayerEngine): a 200 Hz timer (PIT 0x1745)
decrements a prescaler; on underflow the song advances one tick and the
song's tempo word is added to the accumulator. Each channel is a stream of
2-byte events (MIDI-like status in the low byte; note on/off carry one extra
byte) followed by a variable-length wait. Loop markers (start/end measure,
repeat count) replay a section; the song ends when channel 0 reaches its end
marker.

Usage: song-length.py UNPACKED_SONG_FILE...   (e.g. WORMINTR.HSQ unpacked)
"""
import sys

PIT_HZ = 1193182 / 0x1745
CHANNELS = 9
DATA = 2  # song header follows the 2-byte instrument-table offset


def w16(d, o):
    return d[o] | (d[o + 1] << 8)


def read_wait(d, p):
    """Variable-length wait (7 bits per byte, high bit = more), capped at FFFFh."""
    v = 0
    while True:
        b = d[p]
        p += 1
        v = (v << 7) | (b & 0x7F)
        if not b & 0x80:
            break
    return min(v, 0xFFFF), p


def song_seconds(d, play_count=1, max_ticks=10_000_000):
    tempo = w16(d, DATA + 0x30)
    loop_start, loop_end, loop_count = w16(d, DATA + 0x2A), w16(d, DATA + 0x2C), w16(d, DATA + 0x2E)
    start = [0 if w16(d, DATA + 2 * c) == 0 else w16(d, DATA + 2 * c) + DATA for c in range(CHANNELS)]

    def build():
        ptr, wait = start[:], [0xFFFF] * CHANNELS
        for c in range(CHANNELS):
            if ptr[c]:
                wait[c], ptr[c] = read_wait(d, ptr[c])
                wait[c] = (wait[c] + 1) & 0xFFFF
        return ptr, wait

    ptr, wait = build()
    measure, sub, repeat = 1, 0x60, 0
    snap_ptr, snap_wait = ptr[:], wait[:]
    acc, timer_ticks, plays_left = 0, 0, play_count
    while timer_ticks < max_ticks:
        timer_ticks += 1
        hi = ((acc >> 8) - 1) & 0xFF
        acc = (acc & 0xFF) | (hi << 8)
        if not hi & 0x80:
            continue
        # --- one song tick ---
        acc = (acc + tempo) & 0xFFFF
        if repeat == 0:
            if loop_start == measure and sub == 0x60:
                snap_ptr, snap_wait = ptr[:], wait[:]
                repeat = (loop_count - 1) & 0xFFFF
        elif loop_end == measure:
            repeat -= 1
            ptr, wait = snap_ptr[:], snap_wait[:]
            measure = loop_start
        for c in range(CHANNELS):
            wait[c] = (wait[c] - 1) & 0xFFFF
            while wait[c] == 0:
                p = ptr[c]
                if p == 0:
                    break
                ev = w16(d, p)
                p += 2
                handler = (ev >> 4) & 7
                if handler == 7:  # end of track
                    wait[c] = 0xFFFF
                    if c == 0:
                        plays_left -= 1
                        if plays_left == 0:
                            return timer_ticks / PIT_HZ
                        ptr, wait = build()
                        measure, sub = 1, 0x60
                    break
                if handler in (0, 1):  # note off/on: one more data byte
                    p += 1
                wait[c], ptr[c] = read_wait(d, p)
        sub -= 1
        if sub == 0:
            sub, measure = 0x60, measure + 1
    return None


if __name__ == "__main__":
    for path in sys.argv[1:]:
        d = open(path, "rb").read()
        s = song_seconds(d)
        tempo = w16(d, DATA + 0x30)
        loops = (w16(d, DATA + 0x2A), w16(d, DATA + 0x2C), w16(d, DATA + 0x2E))
        name = path.rsplit("/", 1)[-1]
        print(f"{name:14} tempo {tempo:#06x} loop {loops}  length {'∞' if s is None else f'{s:7.2f}s ({int(s // 60)}:{s % 60:05.2f})'}")
