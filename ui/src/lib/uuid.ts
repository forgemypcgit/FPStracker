function hex(byte: number): string {
  return byte.toString(16).padStart(2, '0');
}

// RFC 4122 version 4 UUID with a Safari-friendly fallback.
export function uuidv4(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }

  const bytes = new Uint8Array(16);
  if (typeof crypto !== 'undefined' && typeof crypto.getRandomValues === 'function') {
    crypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }

  // Per RFC 4122: set version and variant bits.
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  const b = Array.from(bytes);
  return [
    b.slice(0, 4).map(hex).join(''),
    b.slice(4, 6).map(hex).join(''),
    b.slice(6, 8).map(hex).join(''),
    b.slice(8, 10).map(hex).join(''),
    b.slice(10, 16).map(hex).join(''),
  ].join('-');
}

