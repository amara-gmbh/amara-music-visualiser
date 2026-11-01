/** Local-only playback, capture and music analysis for the browser visualiser. */
export function localSystemAudioAvailable(hostname) {
  return ['127.0.0.1', 'localhost', '::1', '[::1]'].includes(hostname);
}

export function createAudioEngine() {
  const audio = new Audio();
  audio.preload = 'metadata';
  let context;
  let analyser;
  let fileSource;
  let streamSource;
  let captureStream;
  let systemSession;
  let pendingSystem;
  let sourceKind = 'none';
  let requestGeneration = 0;
  let data;
  let objectUrl;
  let fileName = '';
  let error = '';
  let lastTime = 0;
  let levelAverage = 0;
  let beatEnvelope = 0;
  let previousBass = 0;
  let destroyed = false;
  const spectrum = new Float32Array(48);
  const features = { bass: 0, mids: 0, treble: 0, energy: 0, beat: 0, spectrum };

  function ensureContext() {
    if (destroyed) throw new Error('The audio player has been closed.');
    if (context) return;
    const AudioContextClass = window.AudioContext || window.webkitAudioContext;
    if (!AudioContextClass) throw new Error('This browser does not support audio playback and analysis.');
    context = new AudioContextClass();
    analyser = context.createAnalyser();
    analyser.fftSize = 4096;
    analyser.smoothingTimeConstant = 0;
    data = new Float32Array(analyser.frequencyBinCount);
    fileSource = context.createMediaElementSource(audio);
    // A microphone or shared stream must never be routed to the speakers:
    // doing so would cause feedback and could echo private audio. Only the
    // browser's own file player is connected to the output device.
    fileSource.connect(analyser);
    fileSource.connect(context.destination);
  }

  function releaseCapture() {
    const oldStream = captureStream;
    captureStream = undefined;
    if (streamSource) streamSource.disconnect();
    streamSource = undefined;
    if (sourceKind === 'microphone' || sourceKind === 'shared') sourceKind = 'none';
    if (oldStream) oldStream.getTracks().forEach(track => track.stop());
  }

  function releaseSystemAudio() {
    const old = systemSession;
    systemSession = undefined;
    if (!old) return;
    old.controller.abort();
    old.node.disconnect();
    analyser?.disconnect(old.silent);
    old.silent.disconnect();
    if (sourceKind === 'system') sourceKind = 'none';
  }

  function cancelPendingSystemAudio() {
    pendingSystem?.abort();
    pendingSystem = undefined;
  }

  function stopCapture() {
    requestGeneration += 1;
    cancelPendingSystemAudio();
    releaseSystemAudio();
    releaseCapture();
    error = '';
  }

  function captureError(cause, kind) {
    if (cause?.name === 'NotAllowedError' || cause?.name === 'SecurityError') {
      return kind === 'microphone'
        ? 'Microphone access was not granted.'
        : 'Audio sharing was not granted.';
    }
    if (cause?.name === 'AbortError') return 'Audio sharing was cancelled.';
    if (cause?.name === 'NotFoundError' || cause?.name === 'DevicesNotFoundError') {
      return kind === 'microphone' ? 'No microphone was found.' : 'No shared audio source was found.';
    }
    if (cause?.name === 'NotReadableError') return 'The audio source could not be opened.';
    return kind === 'microphone'
      ? 'Could not start the microphone.'
      : 'Could not start shared audio in this browser.';
  }

  async function startCapture(kind) {
    if (destroyed) throw new Error('The audio player has been closed.');
    const devices = navigator.mediaDevices;
    const request = kind === 'microphone' ? devices?.getUserMedia : devices?.getDisplayMedia;
    if (!request) {
      error = kind === 'microphone'
        ? 'Microphone capture is unavailable in this browser or context.'
        : 'Audio sharing is unavailable in this browser or context.';
      throw new Error(error);
    }
    const generation = ++requestGeneration;
    cancelPendingSystemAudio();
    error = '';
    let stream;
    try {
      ensureContext();
      // Both calls happen in the click handler before the first await. Screen
      // capture requires video as well as audio; the video is never displayed,
      // saved or sent anywhere, but is kept live until sharing is stopped.
      const resume = context.state === 'suspended' ? context.resume() : Promise.resolve();
      const capture = request.call(devices, kind === 'microphone'
        ? { audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }, video: false }
        : { video: true, audio: true });
      const [resumeResult, captureResult] = await Promise.allSettled([resume, capture]);
      if (captureResult.status === 'fulfilled') stream = captureResult.value;
      if (generation !== requestGeneration || destroyed) {
        stream?.getTracks().forEach(track => track.stop());
        return;
      }
      if (captureResult.status === 'rejected') throw captureResult.reason;
      if (resumeResult.status === 'rejected') throw resumeResult.reason;
      if (!stream?.getAudioTracks().some(track => track.readyState === 'live')) {
        stream?.getTracks().forEach(track => track.stop());
        stream = undefined;
        error = kind === 'shared'
          ? 'No audio was shared. Choose a source with audio and enable its audio option.'
          : 'The microphone did not provide an audio track.';
        throw new Error(error);
      }
      // Build the new input before releasing the old one so failed capture
      // leaves the previous source usable.
      const newSource = context.createMediaStreamSource(stream);
      newSource.connect(analyser);
      audio.pause();
      releaseSystemAudio();
      releaseCapture();
      captureStream = stream;
      streamSource = newSource;
      sourceKind = kind;
      error = '';
      stream.getTracks().forEach(track => {
        track.addEventListener('ended', () => {
          if (captureStream !== stream) return;
          releaseCapture();
          error = kind === 'microphone' ? 'Microphone access ended.' : 'Audio sharing ended.';
        }, { once: true });
      });
    } catch (cause) {
      if (stream && stream !== captureStream) stream.getTracks().forEach(track => track.stop());
      if (generation !== requestGeneration || destroyed) return;
      if (!error) error = captureError(cause, kind);
      throw new Error(error, { cause });
    }
  }

  function startMicrophone() { return startCapture('microphone'); }
  function startSharedAudio() { return startCapture('shared'); }

  function systemAudioEndpoint() {
    const windows = navigator.userAgentData?.platform === 'Windows' || /Win/.test(navigator.platform || navigator.userAgent || '');
    return windows ? 'http://127.0.0.1:8769/api/system-audio/stream' : '/api/system-audio/stream';
  }

  // This endpoint exists only when our local, audio-only system bridge is running.
  // Raw samples are held in bounded memory and never played, saved or uploaded.
  async function startSystemAudio() {
    if (destroyed) throw new Error('The audio player has been closed.');
    const generation = ++requestGeneration;
    cancelPendingSystemAudio();
    const controller = new AbortController();
    pendingSystem = controller;
    error = '';
    let node;
    let silent;
    try {
      ensureContext();
      if (!context.audioWorklet || typeof AudioWorkletNode === 'undefined') {
        throw new Error('This browser cannot receive computer audio.');
      }
      // Resume while the click is still a user gesture.
      const resume = context.state === 'suspended' ? context.resume() : Promise.resolve();
      const moduleUrl = new URL('./system-audio-worklet.js', import.meta.url);
      const revision = new URL(import.meta.url).searchParams.get('v');
      if (revision) moduleUrl.searchParams.set('v', revision);
      const module = context.audioWorklet.addModule(moduleUrl.href);
      const responsePromise = fetch(systemAudioEndpoint(), {
        method: 'POST', body: '', signal: controller.signal, cache: 'no-store',
      });
      const [response] = await Promise.all([responsePromise, module, resume]);
      if (generation !== requestGeneration || destroyed) { controller.abort(); return; }
      if (!response.ok) {
        let detail = '';
        try { detail = (await response.json()).error || ''; } catch { /* no JSON error */ }
        throw new Error(detail || `Computer audio is unavailable (${response.status}).`);
      }
      if (!response.body) throw new Error('The computer audio stream is unavailable.');
      node = new AudioWorkletNode(context, 'amara-system-audio', {
        numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [1],
      });
      silent = context.createGain();
      silent.gain.value = 0;
      node.connect(analyser);
      analyser.connect(silent);
      silent.connect(context.destination);
      if (generation !== requestGeneration || destroyed) {
        node.disconnect(); analyser.disconnect(silent); silent.disconnect(); controller.abort(); return;
      }
      // Keep a previous input playing until the new input is actually ready.
      audio.pause();
      releaseSystemAudio();
      releaseCapture();
      const session = { controller, node, silent, lastChunk: 0, lastSignal: 0, inFlight: false };
      node.port.onmessage = () => { session.inFlight = false; };
      systemSession = session;
      pendingSystem = undefined;
      sourceKind = 'system';
      error = '';
      void pumpSystemAudio(response.body.getReader(), session);
    } catch (cause) {
      controller.abort();
      if (node) { node.disconnect(); analyser?.disconnect(silent); }
      silent?.disconnect();
      if (pendingSystem === controller) pendingSystem = undefined;
      if (generation !== requestGeneration || destroyed) return;
      error = cause?.name === 'AbortError' ? 'Computer audio connection stopped.'
        : cause?.message || 'Could not connect to computer audio.';
      throw new Error(error, { cause });
    }
  }

  async function pumpSystemAudio(reader, session) {
    let carry = new Uint8Array(0);
    try {
      while (systemSession === session) {
        const { value, done } = await reader.read();
        if (done) throw new Error('Computer audio disconnected.');
        if (!value?.length) continue;
        const bytes = new Uint8Array(carry.length + value.length);
        bytes.set(carry); bytes.set(value, carry.length);
        const samples = new Float32Array(Math.floor(bytes.length / 4));
        const view = new DataView(bytes.buffer);
        let signal = false;
        for (let i = 0; i < samples.length; i++) {
          const sample = view.getFloat32(i * 4, true);
          samples[i] = Number.isFinite(sample) ? sample : 0;
          signal ||= Math.abs(samples[i]) > 0.00001;
        }
        carry = bytes.slice(samples.length * 4);
        if (samples.length) {
          session.lastChunk = performance.now();
          if (signal) session.lastSignal = session.lastChunk;
          // If the worklet is stalled, drop audio rather than queueing it in
          // the MessagePort indefinitely or allowing large latency to build.
          if (!session.inFlight) {
            session.inFlight = true;
            session.node.port.postMessage(samples, [samples.buffer]);
          }
        }
      }
    } catch (cause) {
      if (systemSession === session) {
        releaseSystemAudio();
        error = cause?.message || 'Computer audio disconnected.';
      }
    } finally {
      try { await reader.cancel(); } catch { /* aborted stream */ }
    }
  }

  async function loadFile(file) {
    if (!file) return;
    if (destroyed) throw new Error('The audio player has been closed.');
    stopCapture();
    audio.pause();
    const previousUrl = objectUrl;
    objectUrl = URL.createObjectURL(file);
    fileName = file.name;
    sourceKind = 'file';
    error = '';
    audio.src = objectUrl;
    audio.load();
    if (previousUrl) URL.revokeObjectURL(previousUrl);
  }

  async function togglePlayback() {
    if (!objectUrl) throw new Error('Choose a track first.');
    if (!audio.paused && !audio.ended) {
      audio.pause();
      return;
    }
    try {
      stopCapture();
      sourceKind = 'file';
      ensureContext();
      if (audio.ended) audio.currentTime = 0;
      // Start both while the click is still a user gesture.
      const resume = context.state === 'suspended' ? context.resume() : Promise.resolve();
      const play = audio.play();
      await Promise.all([resume, play]);
      error = '';
    } catch (cause) {
      error = cause?.name === 'NotSupportedError'
        ? 'This browser cannot play that audio file.'
        : 'Could not play this track. Please try another file.';
      throw new Error(error, { cause });
    }
  }

  function seekFile(seconds) {
    if (sourceKind !== 'file' || !objectUrl || !Number.isFinite(seconds)
        || !Number.isFinite(audio.duration) || audio.duration <= 0) return;
    audio.currentTime = Math.max(0, Math.min(seconds, audio.duration));
  }

  function stopFile() {
    if (sourceKind !== 'file' || !objectUrl) return;
    audio.pause();
    audio.currentTime = 0;
  }

  audio.addEventListener('error', () => {
    if (objectUrl) error = 'This browser cannot play that audio file.';
  });

  // Frequency-band levels are obtained from the actual playing audio. Exponential
  // attack/release makes the visual response stable across different frame rates.
  function getFeatures() {
    const now = performance.now();
    const dt = lastTime ? Math.min((now - lastTime) / 1000, 0.12) : 1 / 60;
    lastTime = now;
    const captureLive = captureStream?.getAudioTracks().some(track => track.readyState === 'live');
    const filePlaying = sourceKind === 'file' && !audio.paused && !audio.ended;
    const systemLive = sourceKind === 'system' && systemSession && systemSession.lastChunk > 0 && now - systemSession.lastChunk < 2000;
    const active = analyser && context.state === 'running' && (captureLive || filePlaying || systemLive);
    if (active) analyser.getFloatFrequencyData(data);
    const sampleRate = context?.sampleRate || 48000;
    const nyquist = sampleRate / 2;
    const upper = Math.min(16000, nyquist * 0.97);
    const ratio = upper / 35;
    for (let i = 0; i < spectrum.length; i += 1) {
      const loHz = 35 * Math.pow(ratio, i / spectrum.length);
      const hiHz = 35 * Math.pow(ratio, (i + 1) / spectrum.length);
      const first = Math.max(1, Math.floor(loHz / nyquist * (data?.length || 0)));
      const end = Math.min(data?.length || 0, Math.max(first + 1, Math.ceil(hiHz / nyquist * (data?.length || 0))));
      let sumSquares = 0;
      if (active) {
        for (let bin = first; bin < end; bin += 1) {
          // Average power rather than dB so sparse high-frequency tones remain
          // visible without treating analyser silence as a signal.
          sumSquares += Number.isFinite(data[bin]) ? Math.pow(10, data[bin] / 10) : 0;
        }
      }
      const meanPower = active && end > first ? sumSquares / (end - first) : 0;
      const levelDb = meanPower > 0 ? 10 * Math.log10(meanPower) : -Infinity;
      // A quiet real tone stays visible; values at or below -72 dB stay dark.
      const raw = Math.max(0, Math.min(1, (levelDb + 72) / 54));
      const rate = raw > spectrum[i] ? 12 : 4.5;
      spectrum[i] += (raw - spectrum[i]) * (1 - Math.exp(-rate * dt));
    }
    const band = (start, end) => {
      let total = 0;
      for (let i = start; i < end; i += 1) total += spectrum[i];
      return Math.min(1, total / (end - start));
    };
    features.bass = band(0, 16);
    features.mids = band(16, 33);
    features.treble = band(33, 48);
    features.energy = Math.min(1, features.bass * .34 + features.mids * .36 + features.treble * .30);
    levelAverage += (features.bass - levelAverage) * (1 - Math.exp(-1.2 * dt));
    const onset = Math.max(0, features.bass - previousBass);
    const hit = onset > .012 && features.bass > levelAverage * 1.15 ? Math.min(1, onset * 15) : 0;
    beatEnvelope = Math.max(hit, beatEnvelope * Math.exp(-5 * dt));
    features.beat = beatEnvelope;
    previousBass = features.bass;
    return features;
  }

  function getState() {
    const captureLive = captureStream?.getAudioTracks().some(track => track.readyState === 'live');
    const now = performance.now();
    const systemLive = sourceKind === 'system' && systemSession && systemSession.lastChunk > 0 && now - systemSession.lastChunk < 2000;
    const systemSignal = pendingSystem ? 'connecting' : sourceKind !== 'system' ? 'off'
      : !systemLive ? 'waiting' : systemSession.lastSignal > 0 && now - systemSession.lastSignal < 1500 ? 'live' : 'silent';
    return { playing: Boolean(captureLive || systemLive || (sourceKind === 'file' && !audio.paused && !audio.ended)),
      fileName, error, source: sourceKind, systemSignal, connecting: Boolean(pendingSystem),
      currentTime: Number.isFinite(audio.currentTime) ? audio.currentTime : 0,
      duration: Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : null };
  }

  function destroy() {
    if (destroyed) return;
    destroyed = true;
    stopCapture();
    audio.pause();
    audio.removeAttribute('src');
    audio.load();
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrl = undefined;
    if (context && context.state !== 'closed') void context.close();
  }

  return { loadFile, togglePlayback, stopFile, seekFile, startMicrophone, startSharedAudio, startSystemAudio, stopCapture,
    getFeatures, getState, destroy };
}
