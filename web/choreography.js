// These clocks use physical time and are independent of the GPU frame loop.
export function createWorldBag(random = Math.random) {
  let bag = [];
  let key = '';
  let first = true;
  return {
    next(available, current) {
      const ids = [...new Set(available)];
      const nextKey = [...ids].sort((a, b) => a - b).join(',');
      if (nextKey !== key) { bag = []; key = nextKey; first = true; }
      if (!ids.length || (ids.length === 1 && ids[0] === current)) return null;
      if (!bag.length) {
        bag = first ? ids.filter(id => id !== current) : [...ids];
        first = false;
        for (let i = bag.length - 1; i > 0; i--) {
          const j = Math.floor(random() * (i + 1));
          [bag[i], bag[j]] = [bag[j], bag[i]];
        }
        // A full cycle includes every world, and never repeats the previous
        // world at a cycle boundary. Do not drop the previous world.
        if (bag[bag.length - 1] === current && bag.length > 1) {
          [bag[0], bag[bag.length - 1]] = [bag[bag.length - 1], bag[0]];
        }
      }
      if (bag[bag.length - 1] === current && bag.length > 1) {
        [bag[0], bag[bag.length - 1]] = [bag[bag.length - 1], bag[0]];
      }
      if (bag.length === 1 && bag[0] === current) { bag = []; return this.next(ids, current); }
      return bag.pop();
    },
    reset() { bag = []; key = ''; first = true; },
  };
}

// Delayed GPU compilation may resolve after a newer selection or Auto toggle.
export function createSelectionGate() {
  let pending = null;
  return {
    begin() { pending = Symbol('selection'); return pending; },
    cancel() { pending = null; },
    current(token) { return pending === token; },
    finish(token) { if (pending === token) pending = null; },
    busy() { return pending !== null; },
  };
}

// Each world owns a look lane. Requests made mid-morph wait for its visible
// endpoint instead of replacing that endpoint and causing a discontinuity.
export function createLookJourney(initial, seconds = 7) {
  const lanes = initial.map(look => ({ from: look, to: look, start: -Infinity, queued: null }));
  const blend = (lane, now) => Math.max(0, Math.min(1, (now - lane.start) / seconds));
  function laneFor(world) {
    if (!Number.isInteger(world) || world < 0 || world >= lanes.length) throw new Error('Invalid world');
    return lanes[world];
  }
  function begin(lane, look, now) { lane.from = lane.to; lane.to = look; lane.start = now; }
  return {
    request(world, look, now) {
      if (!Number.isInteger(look) || look < 0 || look >= 10 || !Number.isFinite(now)) throw new Error('Invalid variation');
      const lane = laneFor(world);
      if (look === lane.to) { lane.queued = null; return; }
      if (blend(lane, now) < 1) lane.queued = look;
      else { lane.queued = null; begin(lane, look, now); }
    },
    advance(now) {
      for (const lane of lanes) {
        if (lane.queued !== null && blend(lane, now) >= 1) {
          const look = lane.queued; lane.queued = null;
          begin(lane, look, now);
        }
      }
    },
    state(world, now) {
      const lane = laneFor(world);
      return { from: lane.from, to: lane.to, blend: blend(lane, now), queued: lane.queued, startedAt: lane.start };
    },
    cancelQueued() { for (const lane of lanes) lane.queued = null; },
  };
}

// Math families are a separate native lane from Fractal Bloom's ten looks.
// Music affects the held phrase length and phase, never an in-flight morph's
// endpoints. Browser onset is measured; no unavailable BPM is invented.
export function createMathJourney(random = Math.random) {
  const morphSeconds = 6;
  const bag = createWorldBag(random);
  const forms = { Julia: 0, InversionLace: 1, HarmonicSculpture: 2, VortexWeave: 3 };
  let from = 0, to = 0, morph = morphSeconds, held = 0, phase = 0;
  let selection = 'Journey', queued = null;
  let evidenceTime = 0, energyArea = 0, transientArea = 0;
  let slowEnergy = 0, slowBrightness = 0;
  const unit = value => Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;
  const gain = value => Number.isFinite(value) ? Math.max(0, Math.min(1.5, value)) : 0;
  function state() {
    const t = Math.min(1, Math.max(0, morph / morphSeconds));
    return { from, to, blend: t*t*t*(t*(t*6-15)+10), phase };
  }
  function begin(family) {
    from = to; to = family; morph = 0; held = 0;
    evidenceTime = 0; energyArea = 0; transientArea = 0;
  }
  function envelope(current, target, dt, tau) {
    const decay = Math.exp(-dt / tau);
    return { next: target + (current - target) * decay,
      area: Math.max(0, target * dt + (current - target) * tau * (1 - decay)) };
  }
  return {
    update(features = {}, mappings = {}, form = 'Journey', delta = 0) {
      const dt = Number.isFinite(delta) ? Math.max(0, Math.min(0.25, delta)) : 0;
      if (!dt) return state();
      if (form !== selection) {
        if (form === 'Journey') { queued = null; held = 0; bag.reset(); }
        selection = form;
      }
      const manual = forms[selection];
      if (manual !== undefined) {
        if (manual === to) queued = null;
        else if (morph < morphSeconds) queued = manual;
        else begin(manual);
      }
      const bass = unit(features.bass) * gain(mappings.bass);
      const mids = unit(features.mids) * gain(mappings.mids);
      const highs = unit(features.treble) * gain(mappings.treble);
      const sum = bass + mids + highs;
      const energy = unit(sum / 3);
      const brightness = sum > 0.001 ? unit(highs / sum) : 0;
      const transient = unit(unit(features.beat) * gain(mappings.beat));
      evidenceTime += dt; energyArea += energy * dt; transientArea += transient * dt;
      const energyStep = envelope(slowEnergy, energy, dt, 3);
      const brightStep = envelope(slowBrightness, brightness, dt, 5);
      slowEnergy = energyStep.next; slowBrightness = brightStep.next;
      phase += dt * 0.16 + energyStep.area * 0.035 + brightStep.area * 0.015;
      let remaining = dt;
      while (remaining > 1e-10) {
        if (morph < morphSeconds) {
          const spent = Math.min(remaining, morphSeconds - morph);
          morph += spent; remaining -= spent;
          if (morph >= morphSeconds - 1e-9) {
            morph = morphSeconds;
            if (queued !== null) { const next = queued; queued = null; begin(next); }
          }
        } else if (selection === 'Journey') {
          const averageEnergy = evidenceTime ? energyArea / evidenceTime : 0;
          const averageTransient = evidenceTime ? transientArea / evidenceTime : 0;
          const duration = Math.max(12, Math.min(22, 22 - averageEnergy * 6 - averageTransient * 2));
          const spent = Math.min(remaining, Math.max(0, duration - held));
          held += spent; remaining -= spent;
          if (held >= duration - 1e-9) {
            const family = bag.next([0, 1, 2, 3], to);
            if (family !== null) begin(family);
          }
        } else break;
      }
      return state();
    },
  };
}

// Mirrors the native camera's smoothly eased physical age: first reveal,
// dive, pull back to the whole set, then unfold again in another region.
export function createMandelbrotJourney() {
  let age = 0;
  let velocity = 1;
  let leg = null;
  let returning = false;
  const reset = () => { age = 0; velocity = 1; leg = null; returning = false; };
  function begin(to, seconds) {
    const distance = to - age;
    if (velocity * distance > 0) seconds = Math.min(seconds, 2.9 * Math.abs(distance / velocity));
    leg = { from: age, to, velocity, seconds: Math.max(0.00001, seconds), elapsed: 0 };
  }
  function dive(look) { returning = false; begin(22 + (look * 3 % 5) * 0.5, Math.max(6, 22 + (look * 7 % 5) * 0.75 - age)); }
  return {
    reset,
    advance(dt, look) {
      let remaining = Number.isFinite(dt) ? Math.max(0, Math.min(dt, 0.1)) : 0;
      let newRegion = false;
      while (remaining > 1e-10) {
        if (!leg) {
          const spent = Math.min(remaining, Math.max(0, 10 - age));
          age += spent; velocity = 1; remaining -= spent;
          if (age >= 10) dive(look);
          continue;
        }
        const spent = Math.min(remaining, leg.seconds - leg.elapsed);
        leg.elapsed += spent; remaining -= spent;
        const t = Math.min(1, leg.elapsed / leg.seconds);
        age = Math.max(0, (2*t*t*t-3*t*t+1)*leg.from + (t*t*t-2*t*t+t)*leg.seconds*leg.velocity + (-2*t*t*t+3*t*t)*leg.to);
        velocity = (6*t*t-6*t)*(leg.from-leg.to)/leg.seconds + (3*t*t-4*t+1)*leg.velocity;
        if (leg.elapsed >= leg.seconds - 1e-10) {
          if (returning) { newRegion = true; dive(look); }
          else { returning = true; begin(0, 16 + look % 3); }
        }
      }
      return { age, newRegion };
    },
  };
}
