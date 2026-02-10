import { create } from 'zustand';
import { createJSONStorage, persist } from 'zustand/middleware';

export interface HardwareSpecs {
  gpu: {
    name: string;
    vram_mb: number;
    clock_mhz?: number;
    driver_version?: string;
  };
  cpu: {
    name: string;
    cores: number;
    threads: number;
    frequency_mhz?: number;
  };
  ram: {
    total_mb: number;
    type?: string;
    speed_mhz?: number;
  };
  confidence: number; // 0-1
}

export interface GameInfo {
  id: string;
  name: string;
  cover_url?: string;
  has_benchmark: boolean;
  difficulty?: 'light' | 'medium' | 'heavy' | 'extreme';
  supports_rt?: boolean;
  supports_dlss?: boolean;
  supports_fsr?: boolean;
  anti_cheat_risk?: 'low' | 'medium' | 'high';
  benchmark_notes?: string;
}

export interface BenchmarkData {
  resolution: string;
  preset: string;
  ray_tracing: boolean;
  upscaling: string; // 'None', 'DLSS Quality', etc.
  capture_method: 'in_game_counter' | 'built_in_benchmark' | 'external_tool';
  fps_avg: number;
  fps_1_low?: number;
  fps_01_low?: number;
  test_location?: string;
  anti_cheat_acknowledged: boolean;
  anti_cheat_strict_acknowledged?: boolean;
}

interface TrackerState {
  // Data
  hardware: HardwareSpecs | null;
  selectedGame: GameInfo | null;
  benchmark: BenchmarkData | null;

  // Actions
  setHardware: (specs: HardwareSpecs) => void;
  setGame: (game: GameInfo) => void;
  setBenchmark: (data: BenchmarkData) => void;
  reset: () => void;
}

export const useTrackerStore = create<TrackerState>()(
  persist(
    (set) => ({
      hardware: null,
      selectedGame: null,
      benchmark: null,

      setHardware: (specs) => set({ hardware: specs }),
      setGame: (game) => set({ selectedGame: game }),
      setBenchmark: (data) => set({ benchmark: data }),
      reset: () => set({ 
        hardware: null, 
        selectedGame: null, 
        benchmark: null 
      }),
    }),
    {
      name: 'nexus-tracker-storage',
      storage: createJSONStorage(() => sessionStorage),
      // Keep persisted state minimal and short-lived; hardware can be re-detected.
      partialize: (state) => ({
        selectedGame: state.selectedGame,
        benchmark: state.benchmark,
      }),
    }
  )
);
