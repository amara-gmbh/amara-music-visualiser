const KEY = 'amara.system-audio-intent';

export function createSystemAudioIntent(getStorage) {
  const storage = () => {
    try { return getStorage(); } catch { return undefined; }
  };
  return {
    requested() {
      try { return storage()?.getItem(KEY) === 'requested'; } catch { return false; }
    },
    request() {
      try { storage()?.setItem(KEY, 'requested'); } catch { /* Storage can be disabled. */ }
    },
    clear() {
      try { storage()?.removeItem(KEY); } catch { /* Storage can be disabled. */ }
    },
  };
}
