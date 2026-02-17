import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ChevronRight, Search, ShieldAlert, Gamepad2, Sparkles } from 'lucide-react';
import type { GameInfo } from '@/lib/trackerStore';
import { useTrackerStore } from '@/lib/trackerStore';
import { useGameCatalog } from '@/hooks/useGameCatalog';

type DifficultyTier = 'extreme' | 'heavy' | 'medium' | 'light';

const tierOrder: DifficultyTier[] = ['extreme', 'heavy', 'medium', 'light'];

const tierLabel: Record<DifficultyTier, string> = {
  extreme: 'Extreme',
  heavy: 'Heavy',
  medium: 'Medium',
  light: 'Light',
};

const tierColorClass: Record<DifficultyTier, string> = {
  extreme: 'text-critical',
  heavy: 'text-caution',
  medium: 'text-oracle',
  light: 'text-optimal',
};

const tierBorderClass: Record<DifficultyTier, string> = {
  extreme: 'border-l-critical/50',
  heavy: 'border-l-caution/50',
  medium: 'border-l-oracle/50',
  light: 'border-l-optimal/50',
};

export default function GamePage() {
  const navigate = useNavigate();
  const { hardware, syntheticStepSeen } = useTrackerStore();
  const setGame = useTrackerStore((s) => s.setGame);
  const [query, setQuery] = useState('');
  const { games, isLoading, error } = useGameCatalog();

  useEffect(() => {
    if (!hardware) {
      navigate('/contribute/detect', { replace: true });
    }
  }, [hardware, navigate]);

  useEffect(() => {
    if (hardware && !syntheticStepSeen) {
      // Synthetic baseline comes before game selection. Users can still skip it by continuing
      // from the baseline page with empty values.
      navigate('/contribute/synthetic', { replace: true });
    }
  }, [hardware, navigate, syntheticStepSeen]);

  if (!hardware || !syntheticStepSeen) {
    return null;
  }

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return games;
    return games.filter((g) => g.name.toLowerCase().includes(q));
  }, [games, query]);

  const grouped = useMemo(() => {
    const buckets: Record<DifficultyTier, GameInfo[]> = {
      extreme: [],
      heavy: [],
      medium: [],
      light: [],
    };
    for (const game of filtered) {
      const d = (game.difficulty ?? 'medium') as DifficultyTier;
      buckets[d].push(game);
    }
    for (const tier of tierOrder) {
      buckets[tier].sort((a, b) => a.name.localeCompare(b.name));
    }
    return buckets;
  }, [filtered]);

  const selectGame = (game: GameInfo) => {
    setGame(game);
    navigate('/contribute/benchmark');
  };

  return (
    <div className="animate-soft-slide">
      <div className="mb-6 flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-oracle/10">
          <Gamepad2 className="h-5 w-5 text-oracle" />
        </div>
        <div>
          <h2 className="text-2xl font-semibold text-white">Choose Game</h2>
          <p className="mt-1 text-sm text-silver">
            Select the game you benchmarked. Anti-cheat risks are labeled.
          </p>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="pointer-events-none absolute left-4 top-3.5 h-4 w-4 text-silver/60" />
        <input
          className="input-base py-3 pl-10 text-base"
          placeholder="Search games..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>

      {error && <p className="mt-2 text-xs text-caution">{error}</p>}

      {isLoading ? (
        <div className="mt-6 space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="shimmer h-16 rounded-xl bg-smoke/30" />
          ))}
        </div>
      ) : (
        <div className="mt-6 space-y-6">
          {tierOrder.map((tier) => {
            const items = grouped[tier];
            if (items.length === 0) return null;

            return (
              <div key={tier}>
                <div className="mb-2.5 flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <div className={`h-2 w-2 rounded-full ${tierColorClass[tier]} bg-current`} />
                    <h3
                      className={`text-xs font-bold uppercase tracking-[0.14em] ${tierColorClass[tier]}`}
                    >
                      {tierLabel[tier]}
                    </h3>
                  </div>
                  <span className="text-[11px] text-silver/40">{items.length}</span>
                </div>
                <div className="space-y-1.5">
                  {items.map((game) => (
                    <button
                      key={game.id}
                      type="button"
                      onClick={() => selectGame(game)}
                      className={`spotlight-card flex w-full items-center justify-between gap-3 rounded-xl border-l-2 ${tierBorderClass[tier]} border border-l-2 border-transparent bg-smoke/25 px-4 py-3 text-left transition-all duration-200 hover:border-ash/30 hover:bg-smoke/50`}
                    >
                      <div className="min-w-0">
                        <span className="text-sm font-medium text-white">{game.name}</span>
                        <div className="mt-1 flex flex-wrap gap-2">
                          {game.has_benchmark && (
                            <span className="badge-optimal gap-1">
                              <Sparkles className="h-2.5 w-2.5" />
                              Built-in bench
                            </span>
                          )}
                          {game.anti_cheat_risk === 'high' && (
                            <span className="badge-critical gap-1">
                              <ShieldAlert className="h-2.5 w-2.5" /> Strict AC
                            </span>
                          )}
                          {game.anti_cheat_risk === 'medium' && (
                            <span className="badge-caution">
                              AC Caution
                            </span>
                          )}
                        </div>
                      </div>
                      <ChevronRight className="h-4 w-4 shrink-0 text-ash transition-transform duration-200 group-hover:translate-x-0.5" />
                    </button>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {query.trim().length > 1 && filtered.length === 0 && (
        <div className="mt-6 panel">
          <p className="text-sm text-silver">
            No match for <span className="font-medium text-white">&ldquo;{query.trim()}&rdquo;</span>.
          </p>
          <button
            type="button"
            className="btn-primary mt-4"
            onClick={() =>
              selectGame({
                id: `custom-${query.trim().toLowerCase().replace(/\s+/g, '-')}`,
                name: query.trim(),
                has_benchmark: false,
                difficulty: 'medium',
                anti_cheat_risk: 'medium',
              })
            }
          >
            Use as Custom Game
          </button>
        </div>
      )}
    </div>
  );
}
