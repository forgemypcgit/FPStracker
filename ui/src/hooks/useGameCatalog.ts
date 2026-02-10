import { useEffect, useMemo, useState } from 'react';
import type { GameInfo } from '@/lib/trackerStore';

interface GameApiResponse {
  id: string;
  name: string;
  has_benchmark: boolean;
  difficulty: 'light' | 'medium' | 'heavy' | 'extreme' | string;
  supports_rt?: boolean;
  supports_dlss?: boolean;
  supports_fsr?: boolean;
  anti_cheat_risk?: 'low' | 'medium' | 'high' | string;
  benchmark_notes?: string;
}

const fallbackGames: GameInfo[] = [
  {
    id: 'cyberpunk-2077',
    name: 'Cyberpunk 2077',
    has_benchmark: true,
    difficulty: 'extreme',
    supports_rt: true,
    supports_dlss: true,
    supports_fsr: true,
    anti_cheat_risk: 'low',
  },
  {
    id: 'alan-wake-2',
    name: 'Alan Wake 2',
    has_benchmark: false,
    difficulty: 'extreme',
    supports_rt: true,
    supports_dlss: true,
    supports_fsr: true,
    anti_cheat_risk: 'low',
  },
  {
    id: 'fortnite',
    name: 'Fortnite',
    has_benchmark: false,
    difficulty: 'medium',
    supports_rt: true,
    supports_dlss: true,
    supports_fsr: true,
    anti_cheat_risk: 'medium',
  },
  {
    id: 'counter-strike-2',
    name: 'Counter-Strike 2',
    has_benchmark: false,
    difficulty: 'light',
    supports_rt: false,
    supports_dlss: false,
    supports_fsr: true,
    anti_cheat_risk: 'medium',
  },
  {
    id: 'valorant',
    name: 'Valorant',
    has_benchmark: false,
    difficulty: 'light',
    supports_rt: false,
    supports_dlss: false,
    supports_fsr: false,
    anti_cheat_risk: 'high',
  },
  {
    id: 'league-of-legends',
    name: 'League of Legends',
    has_benchmark: false,
    difficulty: 'light',
    supports_rt: false,
    supports_dlss: false,
    supports_fsr: false,
    anti_cheat_risk: 'high',
  },
];

const normalizeDifficulty = (value: string): GameInfo['difficulty'] => {
  if (value === 'light' || value === 'medium' || value === 'heavy' || value === 'extreme') {
    return value;
  }
  return 'medium';
};

const normalizeRisk = (value: string | undefined): GameInfo['anti_cheat_risk'] => {
  if (value === 'high' || value === 'medium' || value === 'low') {
    return value;
  }
  return 'medium';
};

export function useGameCatalog() {
  const [games, setGames] = useState<GameInfo[]>(fallbackGames);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const controller = new AbortController();

    const fetchGames = async () => {
      setIsLoading(true);
      setError(null);
      try {
        const response = await fetch('/api/games/list', { signal: controller.signal });
        if (!response.ok) {
          throw new Error(`Game endpoint returned ${response.status}`);
        }

        const payload = (await response.json()) as GameApiResponse[];
        const mapped: GameInfo[] = payload.map((game) => ({
          id: game.id,
          name: game.name,
          has_benchmark: game.has_benchmark,
          difficulty: normalizeDifficulty(game.difficulty),
          supports_rt: Boolean(game.supports_rt),
          supports_dlss: Boolean(game.supports_dlss),
          supports_fsr: Boolean(game.supports_fsr),
          anti_cheat_risk: normalizeRisk(game.anti_cheat_risk),
          benchmark_notes: game.benchmark_notes ?? '',
        }));

        if (mapped.length > 0) {
          setGames(mapped);
        } else {
          setError('Game API returned no entries; using fallback list.');
        }
      } catch (fetchError) {
        if ((fetchError as Error).name === 'AbortError') return;
        setError('Could not load full game catalog. Using fallback list.');
      } finally {
        setIsLoading(false);
      }
    };

    void fetchGames();
    return () => controller.abort();
  }, []);

  const strictCount = useMemo(
    () => games.filter((game) => game.anti_cheat_risk === 'high').length,
    [games]
  );

  return { games, isLoading, error, strictCount };
}
