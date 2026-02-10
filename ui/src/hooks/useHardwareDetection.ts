import { useState, useCallback } from 'react';
import { HardwareSpecs } from '@/lib/trackerStore';

interface HardwareApiResponse {
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
    // Minimal fallback only. Avoid high-entropy browser fingerprinting (e.g. WEBGL_debug_renderer_info).
    const gpuName = 'Unknown GPU (browser fallback)';

    const cores = navigator.hardwareConcurrency || 4;
    // @ts-expect-error deviceMemory is available in modern Chromium-based browsers.
    const ramGB = navigator.deviceMemory || 8;

    return {
      gpu: {
        name: gpuName,
        vram_mb: 0,
      },
      cpu: {
        name: `Generic ${cores}-Core Processor`,
        cores,
        threads: cores * 2,
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
        gpu: {
          name: payload.gpu.name || 'Unknown GPU',
          vram_mb: Number(payload.gpu.vram_mb ?? 0),
        },
        cpu: {
          name: payload.cpu.name || 'Unknown CPU',
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
