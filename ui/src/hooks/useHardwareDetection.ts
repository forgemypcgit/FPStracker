import { useState, useCallback } from 'react';
import { HardwareSpecs } from '@/lib/trackerStore';

interface HardwareApiResponse {
  os_family?: 'windows' | 'linux' | 'macos' | 'other';
  os?: string;
  os_version?: string;
  gpu: {
    name: string;
    vram_mb?: number;
  };
  cpu: {
    name: string;
    cores?: number;
    threads?: number;
  };
  ram: {
    total_mb?: number;
    speed_mhz?: number;
  };
  confidence: number;
}

export function useHardwareDetection() {
  const [isDetecting, setIsDetecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const detectViaBrowser = (): HardwareSpecs => {
    // Minimal fallback only. Do not synthesize hardware model names.
    const logicalThreads = navigator.hardwareConcurrency || 0;
    // @ts-expect-error deviceMemory is available in modern Chromium-based browsers.
    const ramGB = navigator.deviceMemory || 0;

    return {
      gpu: {
        name: '',
        vram_mb: 0,
      },
      cpu: {
        name: '',
        // Browser API exposes logical processors only; keep cores unknown for manual confirmation.
        cores: 0,
        threads: logicalThreads,
      },
      ram: {
        total_mb: ramGB * 1024,
      },
      confidence: 0.3,
    };
  };

  const detectHardware = useCallback(async (): Promise<HardwareSpecs> => {
    setIsDetecting(true);
    setError(null);

    try {
      const response = await fetch('/api/hardware/detect', {
        method: 'POST',
      });

      if (!response.ok) {
        throw new Error(`Hardware endpoint returned ${response.status}`);
      }

      const payload = (await response.json()) as HardwareApiResponse;
      return {
        os_family: payload.os_family,
        os: payload.os,
        os_version: payload.os_version,
        gpu: {
          name: payload.gpu.name || '',
          vram_mb: Number(payload.gpu.vram_mb ?? 0),
        },
        cpu: {
          name: payload.cpu.name || '',
          cores: Number(payload.cpu.cores ?? 0),
          threads: Number(payload.cpu.threads ?? 0),
        },
        ram: {
          total_mb: Number(payload.ram.total_mb ?? 0),
          speed_mhz: payload.ram.speed_mhz,
        },
        confidence: Number.isFinite(payload.confidence) ? payload.confidence : 0.8,
      };
    } catch (apiError) {
      try {
        const fallback = detectViaBrowser();
        setError('Using browser-level detection fallback. Please verify fields.');
        return fallback;
      } catch (fallbackError) {
        const message =
          apiError instanceof Error ? apiError.message : 'Detection failed';
        setError(`Failed to detect hardware. ${message}`);
        return {
          gpu: { name: '', vram_mb: 0 },
          cpu: { name: '', cores: 0, threads: 0 },
          ram: { total_mb: 0 },
          confidence: 0,
        };
      }
    } finally {
      setIsDetecting(false);
    }
  }, []);

  return { detectHardware, isDetecting, error };
}
