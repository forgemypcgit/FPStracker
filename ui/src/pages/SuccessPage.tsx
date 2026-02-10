import { Link, useLocation, useNavigate } from "react-router-dom";
import { CheckCircle2, RotateCcw, Home } from "lucide-react";
import { useTrackerStore } from "@/lib/trackerStore";

interface SuccessLocationState {
  submissionId?: string;
  status?: string;
  message?: string;
}

export default function SuccessPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const { selectedGame, reset } = useTrackerStore();
  const state = (location.state as SuccessLocationState | null) ?? null;
  const isQueued = state?.status?.toLowerCase() === 'queued';
  const idLabel = isQueued ? 'Queued ID' : 'ID';

  const startAnother = () => {
    reset();
    navigate("/detect");
  };

  return (
    <div className="page-wrap animate-soft-slide">
      <section className="mx-auto max-w-lg pt-8 text-center">
        {/* Animated success icon */}
        <div className="relative mx-auto mb-6 flex h-20 w-20 items-center justify-center">
          {/* Radiating rings */}
          <span className="absolute inset-0 rounded-full bg-optimal/10 animate-ring-expand" />
          <span className="absolute inset-2 rounded-full bg-optimal/5 animate-ring-expand" style={{ animationDelay: '0.3s' }} />
          {/* Icon */}
          <div className="relative flex h-16 w-16 items-center justify-center rounded-full bg-optimal/10 shadow-optimal animate-scale-in">
            <CheckCircle2 className="h-8 w-8 text-optimal" />
          </div>
        </div>

        <h1 className="text-3xl font-bold text-white animate-fade-in">Done</h1>
        <p className="mt-3 text-sm leading-relaxed text-silver animate-fade-in-up" style={{ animationDelay: '0.15s' }}>
          Thanks for contributing{selectedGame ? ` ${selectedGame.name} ` : " "}benchmark data.
          It will help improve FPS predictions for future builds.
        </p>

        {(state?.status || state?.message) && (
          <div
            className={`mt-6 rounded-2xl border px-5 py-4 text-left animate-fade-in-up ${
              isQueued
                ? "border-caution/30 bg-caution/[0.04]"
                : "border-optimal/20 bg-optimal/[0.04]"
            }`}
            style={{ animationDelay: "0.22s" }}
          >
            <p
              className={`text-[10px] font-semibold uppercase tracking-wider ${
                isQueued ? "text-caution" : "text-optimal"
              }`}
            >
              {isQueued ? "Queued" : "Submitted"}
            </p>
            {state?.message && <p className="mt-2 text-sm text-silver">{state.message}</p>}
            {isQueued && (
              <p className="mt-2 text-xs text-silver/70">
                Your benchmark was saved locally and will be retried automatically the next time
                you open FPS Tracker (or when you submit again and the backend is reachable).
              </p>
            )}
          </div>
        )}

        {state?.submissionId && (
          <div className="mt-5 inline-flex items-center gap-2 rounded-full bg-smoke/40 px-4 py-1.5 animate-fade-in-up" style={{ animationDelay: '0.3s' }}>
            <span className="text-[10px] font-semibold uppercase tracking-wider text-silver/60">
              {idLabel}
            </span>
            <span className="font-mono text-xs text-silver">{state.submissionId}</span>
          </div>
        )}

        <div className="mt-10 flex flex-wrap justify-center gap-3 animate-fade-in-up" style={{ animationDelay: '0.4s' }}>
          <button type="button" className="btn-primary group" onClick={startAnother}>
            <RotateCcw className="h-4 w-4" /> Submit Another
          </button>
          <Link to="/" className="btn-secondary">
            <Home className="h-4 w-4" /> Home
          </Link>
        </div>
      </section>
    </div>
  );
}
