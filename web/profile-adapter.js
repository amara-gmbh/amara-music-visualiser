// Snapshot the choice the user sees, including a manual selection queued
// behind an unfinished transition. Preserve unrelated native fields intact.
export function snapshotProfile({ base, name, selection, current, next, lookTargets, controls }) {
  const profile = structuredClone(base);
  const selected = selection === 'auto' ? (next ?? current) : Number(selection);
  if (!Number.isInteger(selected) || selected < 0 || selected >= lookTargets.length) {
    throw new Error('Choose an available world');
  }
  profile.name = name;
  profile.visual.scene = selected;
  profile.visual.look = lookTargets[selected];
  profile.visual.motion = Number(controls.pace.value);
  profile.visual.intensity = Number(controls.intensity.value);
  profile.visual.complexity = Number(controls.detail.value);
  profile.visual.graphics_quality = controls.graphics.value;
  profile.mappings.bass_to_pulse = Number(controls.bass.value);
  profile.mappings.mids_to_geometry = Number(controls.mids.value);
  profile.mappings.treble_to_sparkle = Number(controls.highs.value);
  return profile;
}
