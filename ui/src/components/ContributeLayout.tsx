import { Outlet, useLocation } from "react-router-dom";
import { TerminalLayout } from "@/components/layout";

const steps = [
  { path: "/contribute/detect", label: "Detect", num: 1 },
  { path: "/contribute/synthetic", label: "Baseline", num: 2 },
  { path: "/contribute/game", label: "Game", num: 3 },
  { path: "/contribute/benchmark", label: "Results", num: 4 },
  { path: "/contribute/review", label: "Review", num: 5 },
];

export default function ContributeLayout() {
  const location = useLocation();
  const currentIndex = Math.max(
    0,
    steps.findIndex((s) => location.pathname.startsWith(s.path))
  );

  return (
    <TerminalLayout currentStep={steps[currentIndex]?.num ?? 1}>
      <Outlet />
    </TerminalLayout>
  );
}
