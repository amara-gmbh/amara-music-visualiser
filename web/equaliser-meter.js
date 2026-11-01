/** Bounded display geometry for the DOM equaliser. */
export function meterLevel(value, floor = 0.025) {
  const finite = Number.isFinite(value) ? value : 0;
  return Math.min(1, Math.max(floor, finite));
}

export function meterPeakBottom(value) {
  return `calc(${meterLevel(value, 0) * 100}% - 1px)`;
}
