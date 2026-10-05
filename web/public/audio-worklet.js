// Plays the emulator's audio: chunks of interleaved stereo f32 (48 kHz) posted
// by the page. Keeps a small FIFO; starts once ~85 ms are buffered and trims
// if the backlog grows, so latency stays bounded.
const START_FRAMES = 4096;
const MAX_FRAMES = 16384;

class DuneAudio extends AudioWorkletProcessor {
  constructor() {
    super();
    this.chunks = [];
    this.offset = 0; // frames consumed from chunks[0]
    this.frames = 0; // frames buffered
    this.started = false;
    this.port.onmessage = (e) => {
      this.chunks.push(e.data);
      this.frames += e.data.length / 2;
      while (this.frames > MAX_FRAMES && this.chunks.length > 1) {
        const c = this.chunks.shift();
        this.frames -= c.length / 2 - this.offset;
        this.offset = 0;
      }
    };
  }

  process(_inputs, outputs) {
    const [left, right] = outputs[0];
    let i = 0;
    if (!this.started && this.frames >= START_FRAMES) this.started = true;
    while (this.started && i < left.length && this.chunks.length) {
      const c = this.chunks[0];
      const n = Math.min(left.length - i, c.length / 2 - this.offset);
      for (let k = 0; k < n; k++) {
        left[i + k] = c[(this.offset + k) * 2];
        right[i + k] = c[(this.offset + k) * 2 + 1];
      }
      i += n;
      this.offset += n;
      this.frames -= n;
      if (this.offset * 2 >= c.length) {
        this.chunks.shift();
        this.offset = 0;
      }
    }
    if (i < left.length) {
      left.fill(0, i);
      right.fill(0, i);
      if (!this.chunks.length) this.started = false; // underrun: re-buffer
    }
    return true;
  }
}

registerProcessor("dune-audio", DuneAudio);
