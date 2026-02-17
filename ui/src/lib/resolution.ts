export type ResolutionOption = {
  value: string;
  label: string;
};

export const RESOLUTION_OPTIONS: ResolutionOption[] = [
  { value: '1080p', label: '1920x1080 (1080p / FHD)' },
  { value: '1440p', label: '2560x1440 (1440p / 2K / QHD)' },
  { value: '4K', label: '3840x2160 (4K / 2160p / UHD)' },
  { value: '3440x1440', label: '3440x1440 (UWQHD)' },
];

export function formatResolution(value: string): string {
  const trimmed = (value ?? '').trim();
  if (!trimmed) return '—';

  const match = RESOLUTION_OPTIONS.find((opt) => opt.value === trimmed);
  if (match) return match.label;

  const dims = trimmed.match(/^\s*(\d{3,5})\s*x\s*(\d{3,5})\s*$/i);
  if (dims) {
    const width = Number(dims[1]);
    const height = Number(dims[2]);
    if (height === 1080) return `${width}x${height} (1080p)`;
    if (height === 1440) return `${width}x${height} (1440p)`;
    if (height === 2160) return `${width}x${height} (4K)`;
    return `${width}x${height}`;
  }

  return trimmed;
}

