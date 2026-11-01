export const AUTO_WORLD_TRANSITION_SECONDS = 7.5;

export function wallClockProgress(start, now, duration) {
  if (!Number.isFinite(start) || !Number.isFinite(now) || !Number.isFinite(duration) || duration <= 0) return 1;
  return Math.min(1, Math.max(0, (now - start) / duration));
}

export function manualTransitionAnchor(current, next, melt) {
  if (next === null || next === undefined) return current;
  return Number(melt) >= 0.5 ? next : current;
}

export function manualSwitchPlan(current, next, melt, target) {
  return { anchor: manualTransitionAnchor(current, next, melt), target, duration: 0 };
}

export function manualPipelineMasks(worldIds, maskForWorld) {
  return [...new Set(worldIds.map(maskForWorld))];
}

export function createSelectionEventGuard() {
  let lastInput = null;
  return {
    handle(type, value) {
      if (type === 'input') {
        lastInput = value;
        return true;
      }
      if (type === 'change' && lastInput === value) {
        lastInput = null;
        return false;
      }
      lastInput = null;
      return true;
    },
  };
}
