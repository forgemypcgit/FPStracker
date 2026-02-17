function fnv1a32(input: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    // hash *= 16777619 (with overflow)
    hash = (hash + ((hash << 1) + (hash << 4) + (hash << 7) + (hash << 8) + (hash << 24))) >>> 0;
  }
  return hash.toString(16).padStart(8, '0');
}

function storageKey(signature: string): string {
  return `fps-tracker:idem:${fnv1a32(signature)}`;
}

export function getOrCreateIdempotencyKey(signature: string): string {
  const key = storageKey(signature);
  try {
    const existing = localStorage.getItem(key);
    if (existing && existing.trim()) return existing;
  } catch {
    // ignore
  }

  const suffix =
    typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
      ? crypto.randomUUID()
      : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  const created = `fps-tracker-web-${suffix}`;

  try {
    localStorage.setItem(key, created);
  } catch {
    // ignore
  }

  return created;
}

export function clearIdempotencyKey(signature: string): void {
  const key = storageKey(signature);
  try {
    localStorage.removeItem(key);
  } catch {
    // ignore
  }
}

