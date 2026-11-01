// Overall Auto rotates worlds; this separate mode can instead vary the looks
// of one manually held world. The caller owns the smooth look-morph lane.
export function createVariationMode() {
  let automatic = true;
  let selectedAt = 0;
  return {
    isAutomatic() { return automatic; },
    choose(value, now) {
      automatic = value === 'auto';
      selectedAt = now;
    },
    worldChanged(now) { selectedAt = now; },
    shouldAdvance({ selection, current, next, worldStart, look, now }) {
      if (current === 10 || next !== null) return false; // Mandelbrot changes region at its wide pullback.
      if (selection === 'auto') return now - worldStart > 10 && now - look.startedAt > 24;
      return automatic && Number(selection) === current
        && now - Math.max(selectedAt, worldStart, look.startedAt) > 24;
    },
    allowsMandelbrotRegion(selection) {
      return selection === 'auto' || (automatic && Number(selection) === 10);
    },
  };
}
