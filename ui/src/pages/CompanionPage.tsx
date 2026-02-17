import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { Pause, Play, RotateCcw, ArrowLeft, Monitor } from "lucide-react";

function formatElapsed(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

export default function CompanionPage() {
  const [isRunning, setIsRunning] = useState(false);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [targetMinutes, setTargetMinutes] = useState(3);

  const targetSeconds = useMemo(() => Math.max(1, targetMinutes) * 60, [targetMinutes]);
  const progress = useMemo(() => {
    const clamped = Math.min(elapsedSeconds, targetSeconds);
    return clamped / targetSeconds;
  }, [elapsedSeconds, targetSeconds]);

  useEffect(() => {
    if (!isRunning) return;
    const id = window.setInterval(() => {
      setElapsedSeconds((s) => s + 1);
    }, 1000);
    return () => window.clearInterval(id);
  }, [isRunning]);

  useEffect(() => {
    if (elapsedSeconds >= targetSeconds && isRunning) {
      setIsRunning(false);
    }
  }, [elapsedSeconds, isRunning, targetSeconds]);

  return (
    <div className="page-wrap animate-soft-slide">
      <section className="panel-glow">
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-oracle/20 bg-oracle/[0.06] px-3 py-1.5">
              <Monitor className="h-4 w-4 text-oracle" />
              <span className="text-xs font-semibold tracking-wide text-oracle">Companion</span>
            </div>
            <h1 className="mt-3 text-3xl font-semibold text-white">Capture Helper</h1>
            <p className="mt-2 text-sm text-silver">
              An external timer you can keep visible while you record a benchmark. No injection. No hooks.
            </p>
          </div>
          <Link to="/" className="btn-secondary inline-flex items-center gap-2">
            <ArrowLeft className="h-4 w-4" />
            Back
          </Link>
        </div>

        <div className="divider my-6" />

        <div className="grid gap-6 sm:grid-cols-2">
          <div className="rounded-2xl border border-ash/40 bg-smoke/30 p-5">
            <p className="label">Elapsed</p>
            <p className="font-mono text-5xl font-bold text-white">{formatElapsed(elapsedSeconds)}</p>

            <div className="mt-5 h-2 overflow-hidden rounded-full bg-abyss">
              <div
                className="h-full rounded-full bg-oracle transition-all duration-500 ease-out"
                style={{ width: `${Math.round(progress * 100)}%` }}
              />
            </div>
            <p className="mt-2 text-xs text-silver/70">
              Target: {targetMinutes} min ({formatElapsed(targetSeconds)})
            </p>

            <div className="mt-4">
              <label className="label" htmlFor="targetMinutes">
                Target
              </label>
              <select
                id="targetMinutes"
                className="input-base"
                value={targetMinutes}
                onChange={(e) => setTargetMinutes(Number(e.target.value))}
                disabled={isRunning}
              >
                <option value={2}>2 minutes</option>
                <option value={3}>3 minutes</option>
                <option value={5}>5 minutes</option>
                <option value={10}>10 minutes</option>
              </select>
              <p className="mt-2 text-xs text-silver/70">
                You can change this while paused.
              </p>
            </div>

            <div className="mt-6 flex flex-wrap gap-3">
              <button
                type="button"
                className="btn-primary"
                onClick={() => setIsRunning((v) => !v)}
              >
                {isRunning ? (
                  <>
                    <Pause className="h-4 w-4" />
                    Pause
                  </>
                ) : (
                  <>
                    <Play className="h-4 w-4" />
                    Start
                  </>
                )}
              </button>
              <button
                type="button"
                className="btn-secondary"
                onClick={() => {
                  setIsRunning(false);
                  setElapsedSeconds(0);
                }}
              >
                <RotateCcw className="h-4 w-4" />
                Reset
              </button>
            </div>
          </div>

          <div className="rounded-2xl border border-ash/40 bg-smoke/30 p-5">
            <p className="label">Notes</p>
            <ul className="space-y-2 text-sm text-silver">
              <li>
                Use borderless fullscreen if you want this window visible during gameplay.
              </li>
              <li>
                If your OS supports it, you can pin this window as always-on-top.
              </li>
              <li>
                Prefer tool-based capture (CapFrameX / PresentMon) for anti-cheat-sensitive games.
              </li>
              <li>
                When you finish, return to the benchmark flow and submit your numbers.
              </li>
            </ul>

            <div className="mt-5 rounded-xl border border-oracle/20 bg-oracle/[0.05] px-4 py-3 text-xs text-pearl">
              This companion does not read game memory and does not inject into game processes.
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
