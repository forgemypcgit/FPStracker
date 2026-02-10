import { Outlet, Link, useLocation } from "react-router-dom";
import { Check, X } from "lucide-react";

const steps = [
  { path: "/contribute/game", label: "Game", num: 1 },
  { path: "/contribute/benchmark", label: "Results", num: 2 },
  { path: "/contribute/review", label: "Review", num: 3 },
];

export default function ContributeLayout() {
  const location = useLocation();
  const currentIndex = Math.max(
    0,
    steps.findIndex((s) => location.pathname.startsWith(s.path))
  );

  return (
    <div className="page-wrap animate-soft-slide">
      <nav className="mb-10 flex items-center justify-between">
        {/* Stepper */}
        <div className="flex items-center">
          {steps.map((step, i) => {
            const isComplete = i < currentIndex;
            const isCurrent = i === currentIndex;

            return (
              <div key={step.path} className="flex items-center">
                {i > 0 && (
                  <div className="relative mx-2 h-px w-8 overflow-hidden sm:mx-4 sm:w-14">
                    <div
                      className={`absolute inset-0 h-full transition-all duration-500 ease-out ${
                        isComplete
                          ? "bg-gradient-to-r from-oracle to-oracle"
                          : "bg-ash/30"
                      }`}
                    />
                    {isComplete && (
                      <div className="absolute inset-0 h-full bg-gradient-to-r from-oracle/0 via-white/30 to-oracle/0 animate-shimmer" />
                    )}
                  </div>
                )}
                <div className="flex items-center gap-2">
                  <div
                    className={`relative flex h-8 w-8 items-center justify-center rounded-full text-xs font-bold transition-all duration-300 ${
                      isComplete
                        ? "bg-oracle text-void shadow-oracle-subtle"
                        : isCurrent
                        ? "bg-oracle text-void shadow-oracle-subtle"
                        : "border border-ash/60 bg-smoke/40 text-silver"
                    }`}
                  >
                    {isComplete ? (
                      <Check className="h-3.5 w-3.5" strokeWidth={3} />
                    ) : (
                      step.num
                    )}
                    {isCurrent && (
                      <span className="absolute inset-0 rounded-full border-2 border-oracle/30 animate-glow-pulse" />
                    )}
                  </div>
                  <span
                    className={`hidden text-xs font-semibold uppercase tracking-[0.1em] transition-colors duration-300 sm:inline ${
                      isComplete || isCurrent ? "text-white" : "text-silver/50"
                    }`}
                  >
                    {step.label}
                  </span>
                </div>
              </div>
            );
          })}
        </div>

        {/* Exit */}
        <Link
          to="/"
          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-silver transition-all duration-200 hover:bg-smoke/50 hover:text-white"
        >
          <X className="h-3.5 w-3.5" />
          <span className="hidden sm:inline">Exit</span>
        </Link>
      </nav>

      <Outlet />
    </div>
  );
}
