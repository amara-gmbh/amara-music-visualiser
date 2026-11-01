// Local helpers emit 48 kHz mono. Keep a short bounded queue and resample to
// the browser device rate without storing or replaying captured system audio.
class SystemAudioInput extends AudioWorkletProcessor {
  constructor() {
    super();
    this.ring = new Float32Array(8192);
    this.read = 0;
    this.write = 0;
    this.count = 0;
    this.fraction = 0;
    this.port.onmessage = ({ data }) => {
      if (!(data instanceof Float32Array)) return;
      for (const sample of data) {
        if (this.count === this.ring.length) {
          this.read = (this.read + 1) % this.ring.length;
          this.count--;
        }
        // WASAPI mix levels are often deliberately quiet. Apply bounded gain
        // only in the muted analysis path so low-level music remains visible
        // without allowing samples outside the Web Audio [-1, 1] range.
        this.ring[this.write] = Number.isFinite(sample) ? Math.tanh(sample * 16) : 0;
        this.write = (this.write + 1) % this.ring.length;
        this.count++;
      }
      this.port.postMessage('ready');
    };
  }

  process(_inputs, outputs) {
    const channel = outputs[0]?.[0];
    if (!channel) return true;
    const step = 48000 / sampleRate;
    for (let i = 0; i < channel.length; i++) {
      if (this.count < 2) { channel[i] = 0; this.fraction = 0; continue; }
      const a = this.ring[this.read];
      const b = this.ring[(this.read + 1) % this.ring.length];
      channel[i] = a + (b - a) * this.fraction;
      this.fraction += step;
      while (this.fraction >= 1 && this.count > 1) {
        this.read = (this.read + 1) % this.ring.length;
        this.count--;
        this.fraction--;
      }
    }
    return true;
  }
}
registerProcessor('amara-system-audio', SystemAudioInput);
