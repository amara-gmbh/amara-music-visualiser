/** Bounded, local-only musical structure estimates from already-analysed levels. */
const clamp = (value, low = 0, high = 1) => Math.max(low, Math.min(high, value));

export function createMusicInsight() {
  const onsetTimes = new Float64Array(24);
  const intervals = new Float64Array(23);
  const result = { active: false, onset: 0, bpm: 0, tempoConfidence: 0,
    section: 'quiet', trend: 'steady', event: 'none', buildup: 0, drop: 0 };
  let source = 'none';
  let lastNow;
  let previousEnergy = 0;
  let previousBass = 0;
  let fastEnergy = 0;
  let slowEnergy = 0;
  let deviation = 0;
  let silenceSince;
  let lastOnset = -Infinity;
  let onsetCount = 0;
  let onsetHead = 0;
  let risingSince;
  let risingBase = 0;
  let buildupArmed = false;
  let buildupAnnounced = false;
  let dropEnvelope = 0;

  function clearAnalysis(nextSource = source) {
    source = nextSource;
    lastNow = undefined;
    previousEnergy = 0;
    previousBass = 0;
    fastEnergy = 0;
    slowEnergy = 0;
    deviation = 0;
    silenceSince = undefined;
    lastOnset = -Infinity;
    onsetCount = 0;
    onsetHead = 0;
    risingSince = undefined;
    risingBase = 0;
    buildupArmed = false;
    buildupAnnounced = false;
    dropEnvelope = 0;
    Object.assign(result, { active: false, onset: 0, bpm: 0, tempoConfidence: 0,
      section: 'quiet', trend: 'steady', event: 'none', buildup: 0, drop: 0 });
  }

  function orderedOnset(index) {
    const start = onsetCount < onsetTimes.length ? 0 : onsetHead;
    return onsetTimes[(start + index) % onsetTimes.length];
  }

  function updateTempo() {
    const count = Math.min(onsetCount, onsetTimes.length);
    let intervalCount = 0;
    for (let i = 1; i < count; i++) {
      let interval = orderedOnset(i) - orderedOnset(i - 1);
      if (interval < 0.24 || interval > 1.5) continue;
      while (interval < 1 / 3) interval *= 2;
      while (interval > 1) interval /= 2;
      intervals[intervalCount++] = interval;
    }
    if (intervalCount < 3) {
      result.bpm = 0;
      result.tempoConfidence = 0;
      return;
    }
    // In-place insertion sort on a fixed scratch buffer; no unbounded history.
    for (let i = 1; i < intervalCount; i++) {
      const value = intervals[i];
      let j = i - 1;
      while (j >= 0 && intervals[j] > value) { intervals[j + 1] = intervals[j]; j--; }
      intervals[j + 1] = value;
    }
    const median = intervals[Math.floor(intervalCount / 2)];
    let error = 0;
    for (let i = 0; i < intervalCount; i++) error += Math.abs(intervals[i] - median);
    const regularity = clamp(1 - error / intervalCount / Math.max(0.001, median * 0.18));
    const evidence = clamp((intervalCount - 2) / 6);
    result.bpm = Math.round(60 / median);
    result.tempoConfidence = regularity * evidence;
  }

  function recordOnset(now) {
    onsetTimes[onsetHead] = now;
    onsetHead = (onsetHead + 1) % onsetTimes.length;
    onsetCount++;
    lastOnset = now;
    updateTempo();
  }

  function reset(nextSource = 'none') {
    clearAnalysis(nextSource);
    return result;
  }

  function update(input = {}, now = 0) {
    const nextSource = typeof input.source === 'string' ? input.source : 'none';
    if (nextSource !== source) clearAnalysis(nextSource);
    const time = Number.isFinite(now) ? now : (lastNow ?? 0);
    const dt = lastNow === undefined ? 1 / 60 : clamp(time - lastNow, 1 / 240, 0.2);
    lastNow = time;
    result.event = 'none';
    result.onset = 0;
    dropEnvelope *= Math.exp(-1.5 * dt);

    if (!input.active) {
      clearAnalysis(nextSource);
      return result;
    }
    result.active = true;
    const energy = clamp(Number.isFinite(input.energy) ? input.energy : 0);
    const bass = clamp(Number.isFinite(input.bass) ? input.bass : 0);
    if (energy < 0.012) {
      silenceSince ??= time;
      result.section = 'quiet';
      result.trend = 'steady';
      result.buildup = 0;
      result.drop = dropEnvelope;
      if (time - silenceSince >= 1.2) {
        const activeSource = source;
        clearAnalysis(activeSource);
        result.active = true;
      }
      previousEnergy = energy;
      previousBass = bass;
      return result;
    }
    silenceSince = undefined;

    if (fastEnergy === 0 && slowEnergy === 0) fastEnergy = slowEnergy = energy;
    const fastRate = 1 - Math.exp(-4.5 * dt);
    const slowRate = 1 - Math.exp(-0.45 * dt);
    fastEnergy += (energy - fastEnergy) * fastRate;
    slowEnergy += (energy - slowEnergy) * slowRate;
    deviation += (Math.abs(energy - slowEnergy) - deviation) * (1 - Math.exp(-1.2 * dt));

    const novelty = Math.max(0, energy - previousEnergy) * 0.72 + Math.max(0, bass - previousBass) * 0.28;
    const onsetThreshold = Math.max(0.03, deviation * 0.9);
    const onset = time - lastOnset >= 0.22 && energy >= 0.055 && novelty >= onsetThreshold;
    if (onset) {
      result.onset = clamp(novelty * 3.5);
      recordOnset(time);
    }

    const difference = fastEnergy - slowEnergy;
    result.trend = difference > 0.018 ? 'rising' : difference < -0.018 ? 'falling' : 'steady';
    result.section = slowEnergy < 0.07 ? 'low' : slowEnergy < 0.24 ? 'medium' : 'high';

    if (result.trend === 'rising' && energy > 0.1) {
      if (risingSince === undefined) { risingSince = time; risingBase = energy; buildupAnnounced = false; }
      const duration = time - risingSince;
      const rise = Math.max(0, energy - risingBase);
      result.buildup = clamp((duration / 1.2) * (rise / 0.14));
      if (!buildupAnnounced && duration >= 0.8 && rise >= 0.1) {
        result.event = 'buildup';
        buildupArmed = true;
        buildupAnnounced = true;
        result.buildup = Math.max(result.buildup, 0.75);
      }
    } else {
      risingSince = undefined;
      risingBase = energy;
      result.buildup = buildupArmed ? 0.65 : 0;
    }

    if (buildupArmed && onset && energy >= 0.34 && energy - previousEnergy >= 0.08) {
      result.event = 'drop';
      buildupArmed = false;
      buildupAnnounced = false;
      result.buildup = 0;
      dropEnvelope = 1;
    }
    result.drop = dropEnvelope;
    previousEnergy = energy;
    previousBass = bass;
    return result;
  }

  clearAnalysis();
  return { update, reset, get state() { return result; } };
}

export function describeMusicInsight(insight) {
  if (!insight?.active || insight.section === 'quiet') return 'Music insight · waiting for a clear signal';
  const tempo = insight.tempoConfidence >= 0.55 ? `${insight.bpm} BPM` : 'tempo forming';
  const movement = insight.event === 'drop' ? ' · drop' : insight.event === 'buildup' ? ' · buildup'
    : insight.trend !== 'steady' ? ` · ${insight.trend}` : '';
  return `Music insight · ${tempo} · ${insight.section} energy${movement}`;
}

export function musicVisualAdjustments(insight) {
  const buildup = clamp(Number.isFinite(insight?.buildup) ? insight.buildup : 0);
  const drop = clamp(Number.isFinite(insight?.drop) ? insight.drop : 0);
  return { paceScale: 1 + buildup * 0.025,
    intensityScale: 1 + buildup * 0.025 + drop * 0.06,
    glowOffset: buildup * 0.03 + drop * 0.05 };
}
