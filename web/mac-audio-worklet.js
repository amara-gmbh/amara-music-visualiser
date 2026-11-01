// The Core Audio helper emits 48 kHz mono. Keep a short bounded queue and
// resample to the browser device's sample rate without ever storing audio.
class MacAudioInput extends AudioWorkletProcessor {
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
        this.ring[this.write] = Number.isFinite(sample) ? sample : 0;
        this.write = (this.write + 1) % this.ring.length;
        this.count++;
      }
      // The sender keeps at most one unacknowledged message in flight.
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
registerProcessor('amara-mac-audio', MacAudioInput);
