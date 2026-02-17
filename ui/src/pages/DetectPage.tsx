import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Cpu, MemoryStick, Search, Video, Loader2, ArrowRight, HelpCircle, Wrench } from "lucide-react";
import { useHardwareDetection } from "@/hooks/useHardwareDetection";
import { HardwareSpecs, useTrackerStore } from "@/lib/trackerStore";
import type { ReactNode } from "react";

type DepsStatusItem = {
  name: string;
  required: boolean;
  available: boolean;
  details: string;
};

type WindowsRuntimeStatus = {
  winget_available: boolean;
  presentmon_path?: string | null;
  presentmon_help_ok: boolean;
  presentmon_help_summary: string;
};

type DepsStatusResponse = {
  platform: string;
  dependencies: DepsStatusItem[];
  windows_runtime?: WindowsRuntimeStatus | null;
};

const emptySpecs: HardwareSpecs = {
  gpu: { name: "", vram_mb: 0 },
  cpu: { name: "", cores: 0, threads: 0 },
  ram: { total_mb: 0 },
  confidence: 0,
};

export default function DetectPage() {
  const navigate = useNavigate();
  const { detectHardware, isDetecting, error } = useHardwareDetection();
  const { hardware: storedHardware, setHardware } = useTrackerStore();
  const [specs, setSpecs] = useState<HardwareSpecs>(emptySpecs);
  const [deps, setDeps] = useState<DepsStatusResponse | null>(null);
  const [depsError, setDepsError] = useState<string | null>(null);
  const [presentmonInstalling, setPresentmonInstalling] = useState(false);
  const [presentmonInstallError, setPresentmonInstallError] = useState<string | null>(null);
  const [showPresentmonConfirm, setShowPresentmonConfirm] = useState(false);
  const [approvals, setApprovals] = useState({
    gpu: false,
    cpu: false,
    ram: false,
  });

  useEffect(() => {
    if (storedHardware) {
      setSpecs(storedHardware);
      setApprovals({ gpu: false, cpu: false, ram: false });
    }
  }, [storedHardware]);

  async function refreshDeps() {
    setDepsError(null);
    try {
      const res = await fetch("/api/deps/status");
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        throw new Error(text || `Dependency status failed (${res.status})`);
      }
      const data = (await res.json()) as DepsStatusResponse;
      setDeps(data);
    } catch (err) {
      setDeps(null);
      setDepsError(err instanceof Error ? err.message : "Failed to load dependency status.");
    }
  }

  useEffect(() => {
    void refreshDeps();
  }, []);

  const isWindows = deps?.platform === "windows";
  const presentmonPath = deps?.windows_runtime?.presentmon_path ?? null;
  const presentmonMissing = Boolean(isWindows && !presentmonPath);

  const confidencePercent = Math.round(specs.confidence * 100);

  const confidenceLabel = useMemo(() => {
    if (specs.confidence >= 0.85) return "High confidence";
    if (specs.confidence >= 0.6) return "Medium confidence";
    if (specs.confidence > 0) return "Low confidence";
    return "Not detected yet";
  }, [specs.confidence]);

  const confidenceColor = useMemo(() => {
    if (specs.confidence >= 0.85) return "text-optimal";
    if (specs.confidence >= 0.6) return "text-oracle";
    if (specs.confidence > 0) return "text-caution";
    return "text-silver/40";
  }, [specs.confidence]);

  const confidenceStroke = useMemo(() => {
    if (specs.confidence >= 0.85) return "#79f2a6";
    if (specs.confidence >= 0.6) return "#19d4ff";
    if (specs.confidence > 0) return "#ffb454";
    return "#283548";
  }, [specs.confidence]);

  const gpuStatus = useMemo<FieldStatus>(() => {
    const name = specs.gpu.name.trim();
    if (!name) return "missing";
    if (isPlaceholderName(name) || specs.gpu.vram_mb <= 0) return "review";
    return "ok";
  }, [specs.gpu.name, specs.gpu.vram_mb]);

  const cpuStatus = useMemo<FieldStatus>(() => {
    const name = specs.cpu.name.trim();
    if (!name || specs.cpu.cores <= 0 || specs.cpu.threads <= 0) return "missing";
    if (isPlaceholderName(name) || specs.cpu.threads < specs.cpu.cores) return "review";
    return "ok";
  }, [specs.cpu.cores, specs.cpu.name, specs.cpu.threads]);

  const ramStatus = useMemo<FieldStatus>(() => {
    if (specs.ram.total_mb <= 0) return "missing";
    if (specs.ram.total_mb < 4096) return "review";
    return "ok";
  }, [specs.ram.total_mb]);

  const checklist = useMemo(() => {
    const items: string[] = [];
    if (gpuStatus === "missing") items.push("Fill GPU model and VRAM.");
    if (cpuStatus === "missing") items.push("Fill CPU model, cores, and threads.");
    if (ramStatus === "missing") items.push("Fill total RAM amount.");
    if (!approvals.gpu) items.push("Approve GPU values after reviewing.");
    if (!approvals.cpu) items.push("Approve CPU values after reviewing.");
    if (!approvals.ram) items.push("Approve RAM values after reviewing.");
    return items;
  }, [approvals.cpu, approvals.gpu, approvals.ram, cpuStatus, gpuStatus, ramStatus]);

  const canContinue =
    gpuStatus !== "missing" &&
    cpuStatus !== "missing" &&
    ramStatus !== "missing" &&
    approvals.gpu &&
    approvals.cpu &&
    approvals.ram;

  const runDetection = async () => {
    const detected = await detectHardware();
    setSpecs(detected);
    setApprovals({ gpu: false, cpu: false, ram: false });
  };

  async function installPresentmon() {
    setPresentmonInstallError(null);
    setPresentmonInstalling(true);
    try {
      const res = await fetch("/api/deps/presentmon/install", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ confirm: true }),
      });
      const payload = (await res.json().catch(() => null)) as
        | { ok: boolean; presentmon_path?: string; message?: string }
        | null;
      if (!res.ok || !payload || !payload.ok) {
        throw new Error(payload?.message || `Install failed (${res.status})`);
      }
      await refreshDeps();
      setShowPresentmonConfirm(false);
    } catch (err) {
      setPresentmonInstallError(err instanceof Error ? err.message : "Install failed.");
    } finally {
      setPresentmonInstalling(false);
    }
  }

  // SVG gauge constants
  const radius = 40;
  const circumference = 2 * Math.PI * radius;
  const strokeOffset = circumference - (specs.confidence * circumference);

  return (
    <div className="animate-soft-slide">
      {/* Header with gauge */}
      <section className="flex flex-col gap-6 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex items-start gap-5">
          {/* Confidence ring gauge */}
          <div className="relative flex h-24 w-24 shrink-0 items-center justify-center">
            <svg className="gauge-ring h-full w-full" viewBox="0 0 96 96">
              {/* Background ring */}
              <circle
                cx="48" cy="48" r={radius}
                fill="none"
                stroke="rgba(40,53,72,0.4)"
                strokeWidth="5"
              />
              {/* Progress ring */}
              <circle
                cx="48" cy="48" r={radius}
                fill="none"
                stroke={confidenceStroke}
                strokeWidth="5"
                strokeLinecap="round"
                strokeDasharray={circumference}
                strokeDashoffset={strokeOffset}
              />
            </svg>
            <div className="absolute inset-0 flex flex-col items-center justify-center">
              <span className={`font-mono text-lg font-bold ${confidenceColor}`}>
                {confidencePercent}%
              </span>
            </div>
          </div>

          <div>
            <h1 className="text-3xl font-semibold text-white">Hardware Detection</h1>
            <p className="mt-1.5 text-sm text-silver">
              Assisted mode: keep detected values, edit anything suspicious, then approve.
            </p>
            <p className={`mt-2 text-xs font-medium ${confidenceColor}`}>
              {error ? <span className="text-caution">{error}</span> : confidenceLabel}
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={runDetection}
          disabled={isDetecting}
          className="btn-primary shrink-0"
        >
          {isDetecting ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Search className="h-4 w-4" />
          )}
          {isDetecting ? "Scanning..." : "Auto-Detect"}
        </button>
      </section>

      {/* Windows live capture dependency (PresentMon) */}
      {isWindows && (
        <section className="mt-6 rounded-xl border border-ash/20 bg-smoke/20 p-4">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="flex items-start gap-3">
              <div className="mt-0.5 flex h-8 w-8 items-center justify-center rounded-xl bg-oracle/10">
                <Wrench className="h-4 w-4 text-oracle" />
              </div>
              <div>
                <h2 className="text-sm font-semibold text-pearl">Windows live capture tool</h2>
                <p className="mt-1 text-xs text-silver">
                  PresentMon enables automatic frametime capture on Windows. It's optional unless you use live auto-capture.
                </p>
                {presentmonMissing ? (
                  <p className="mt-2 text-xs text-caution">
                    PresentMon not found. You can install it now (recommended).
                  </p>
                ) : (
                  <p className="mt-2 text-xs text-optimal">
                    PresentMon ready: <span className="font-mono">{presentmonPath}</span>
                  </p>
                )}
                {deps?.windows_runtime?.presentmon_help_ok === false && !presentmonMissing && (
                  <p className="mt-2 text-xs text-caution">
                    PresentMon exists but may not be runnable: {deps.windows_runtime?.presentmon_help_summary}
                  </p>
                )}
                {depsError && <p className="mt-2 text-xs text-caution">{depsError}</p>}
              </div>
            </div>

            {presentmonMissing && (
              <div className="flex items-center gap-2">
                {!showPresentmonConfirm ? (
                  <button
                    type="button"
                    className="btn-secondary"
                    onClick={() => setShowPresentmonConfirm(true)}
                    disabled={presentmonInstalling}
                  >
                    Install PresentMon
                  </button>
                ) : (
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      className="btn-secondary"
                      onClick={() => setShowPresentmonConfirm(false)}
                      disabled={presentmonInstalling}
                    >
                      Cancel
                    </button>
                    <button
                      type="button"
                      className="btn-primary"
                      onClick={() => void installPresentmon()}
                      disabled={presentmonInstalling}
                    >
                      {presentmonInstalling ? (
                        <Loader2 className="h-4 w-4 animate-spin" />
                      ) : null}
                      {presentmonInstalling ? "Installing..." : "Confirm install"}
                    </button>
                  </div>
                )}
              </div>
            )}
          </div>

          {presentmonInstallError && (
            <div className="mt-3 rounded-lg border border-critical/30 bg-critical/[0.06] px-3 py-2 text-xs text-pearl">
              {presentmonInstallError}
            </div>
          )}
        </section>
      )}

      <section className="mt-6 rounded-xl border border-ash/20 bg-smoke/20 p-4">
        <h2 className="text-sm font-semibold text-pearl">Assisted Review Checklist</h2>
        <p className="mt-1 text-xs text-silver">
          We preserve detected values. You can edit any field and then confirm each section.
        </p>
        {checklist.length > 0 ? (
          <ul className="mt-3 space-y-1.5 text-xs text-caution">
            {checklist.map((item) => (
              <li key={item}>• {item}</li>
            ))}
          </ul>
        ) : (
          <p className="mt-3 text-xs text-optimal">All required values are filled and approved.</p>
        )}
      </section>

      {/* Spec cards */}
      <section className="stagger-children mt-8 grid gap-4 lg:grid-cols-3">
        <SpecCard
          title="GPU"
          icon={<Video className="h-5 w-5" />}
          accentColor="text-oracle"
          borderColor="border-t-oracle/40"
          isDetecting={isDetecting}
        >
          <StatusBadge status={gpuStatus} />
          <label className="label">Model</label>
          <input
            className="input-base"
            placeholder="e.g. GeForce RTX 4070"
            value={specs.gpu.name}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, gpu: { ...c.gpu, name: e.target.value } }))
            }
            onInput={() => setApprovals((v) => ({ ...v, gpu: false }))}
          />

          <label className="label mt-4">VRAM (MB)</label>
          <input
            className="input-base font-mono"
            type="number"
            min={0}
            value={specs.gpu.vram_mb || ""}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, gpu: { ...c.gpu, vram_mb: Number(e.target.value) || 0 } }))
            }
            onInput={() => setApprovals((v) => ({ ...v, gpu: false }))}
          />
          {specs.gpu.vram_mb > 0 && (
            <p className="mt-1.5 text-xs text-silver/60">
              {(specs.gpu.vram_mb / 1024).toFixed(1)} GB
            </p>
          )}

          <ApprovalToggle
            checked={approvals.gpu}
            onToggle={() => setApprovals((v) => ({ ...v, gpu: !v.gpu }))}
            label="I confirm these GPU values are correct."
          />
          <FieldHelp>
            Windows: open Task Manager (Ctrl+Shift+Esc), then Performance → GPU.
            Use the GPU name and dedicated memory (VRAM) shown there.
          </FieldHelp>
        </SpecCard>

        <SpecCard
          title="CPU"
          icon={<Cpu className="h-5 w-5" />}
          accentColor="text-optimal"
          borderColor="border-t-optimal/40"
          isDetecting={isDetecting}
        >
          <StatusBadge status={cpuStatus} />
          <label className="label">Model</label>
          <input
            className="input-base"
            placeholder="e.g. Ryzen 7 7800X3D"
            value={specs.cpu.name}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, cpu: { ...c.cpu, name: e.target.value } }))
            }
            onInput={() => setApprovals((v) => ({ ...v, cpu: false }))}
          />

          <div className="mt-4 grid grid-cols-2 gap-3">
            <div>
              <label className="label">Cores</label>
              <input
                className="input-base font-mono"
                type="number"
                min={1}
                value={specs.cpu.cores || ""}
                onChange={(e) =>
                  setSpecs((c) => ({ ...c, cpu: { ...c.cpu, cores: Number(e.target.value) || 0 } }))
                }
                onInput={() => setApprovals((v) => ({ ...v, cpu: false }))}
              />
            </div>
            <div>
              <label className="label">Threads</label>
              <input
                className="input-base font-mono"
                type="number"
                min={1}
                value={specs.cpu.threads || ""}
                onChange={(e) =>
                  setSpecs((c) => ({ ...c, cpu: { ...c.cpu, threads: Number(e.target.value) || 0 } }))
                }
                onInput={() => setApprovals((v) => ({ ...v, cpu: false }))}
              />
            </div>
          </div>

          <ApprovalToggle
            checked={approvals.cpu}
            onToggle={() => setApprovals((v) => ({ ...v, cpu: !v.cpu }))}
            label="I confirm these CPU values are correct."
          />
          <FieldHelp>
            Windows: Task Manager → Performance → CPU. Use processor name, core count, and
            logical processors (threads).
          </FieldHelp>
        </SpecCard>

        <SpecCard
          title="RAM"
          icon={<MemoryStick className="h-5 w-5" />}
          accentColor="text-caution"
          borderColor="border-t-caution/40"
          isDetecting={isDetecting}
        >
          <StatusBadge status={ramStatus} />
          <label className="label">Total Memory (MB)</label>
          <input
            className="input-base font-mono"
            type="number"
            min={0}
            value={specs.ram.total_mb || ""}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, ram: { ...c.ram, total_mb: Number(e.target.value) || 0 } }))
            }
            onInput={() => setApprovals((v) => ({ ...v, ram: false }))}
          />
          <p className="mt-1.5 text-xs text-silver/60">
            {(specs.ram.total_mb / 1024 || 0).toFixed(1)} GB
          </p>

          <label className="label mt-4">Speed (MHz)</label>
          <input
            className="input-base font-mono"
            type="number"
            min={0}
            placeholder="Optional"
            value={specs.ram.speed_mhz || ""}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, ram: { ...c.ram, speed_mhz: Number(e.target.value) || 0 } }))
            }
            onInput={() => setApprovals((v) => ({ ...v, ram: false }))}
          />

          <ApprovalToggle
            checked={approvals.ram}
            onToggle={() => setApprovals((v) => ({ ...v, ram: !v.ram }))}
            label="I confirm these RAM values are correct."
          />
          <FieldHelp>
            Windows: Task Manager → Performance → Memory. Use installed memory as total RAM.
            Speed is optional if you cannot find it quickly.
          </FieldHelp>
        </SpecCard>
      </section>

      {/* Continue */}
      <section className="mt-8 flex justify-end">
        <button
          type="button"
          className="btn-primary group"
          disabled={!canContinue}
          onClick={() => {
            setHardware(specs);
            navigate("/contribute/synthetic");
          }}
        >
          Save and Continue
          <ArrowRight className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />
        </button>
      </section>
    </div>
  );
}

interface SpecCardProps {
  title: string;
  icon: ReactNode;
  accentColor: string;
  borderColor: string;
  isDetecting: boolean;
  children: ReactNode;
}

function SpecCard({ title, icon, accentColor, borderColor, isDetecting, children }: SpecCardProps) {
  return (
    <article className={`panel border-t-2 ${borderColor} ${isDetecting ? "shimmer" : ""}`}>
      <div className="mb-4 flex items-center gap-2.5">
        <div className={`flex h-8 w-8 items-center justify-center rounded-lg bg-smoke/60 ${accentColor}`}>
          {icon}
        </div>
        <h2 className="text-lg font-semibold text-white">{title}</h2>
      </div>
      {children}
    </article>
  );
}

type FieldStatus = "ok" | "review" | "missing";

function isPlaceholderName(value: string): boolean {
  const normalized = value.trim().toLowerCase();
  return (
    normalized.length === 0 ||
    normalized === "unknown" ||
    normalized.includes("unknown") ||
    normalized.includes("generic") ||
    normalized.includes("fallback")
  );
}

function StatusBadge({ status }: { status: FieldStatus }) {
  if (status === "ok") {
    return (
      <p className="mb-3 rounded-lg border border-optimal/25 bg-optimal/10 px-2.5 py-1 text-[11px] text-optimal">
        Looks good
      </p>
    );
  }
  if (status === "review") {
    return (
      <p className="mb-3 rounded-lg border border-caution/25 bg-caution/10 px-2.5 py-1 text-[11px] text-caution">
        Review recommended
      </p>
    );
  }
  return (
    <p className="mb-3 rounded-lg border border-critical/25 bg-critical/10 px-2.5 py-1 text-[11px] text-critical">
      Required value missing
    </p>
  );
}

function ApprovalToggle({
  checked,
  onToggle,
  label,
}: {
  checked: boolean;
  onToggle: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={`mt-3 flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left text-xs transition-colors ${
        checked
          ? "border-optimal/35 bg-optimal/10 text-optimal"
          : "border-ash/30 bg-smoke/40 text-silver hover:border-ash/50"
      }`}
    >
      <span
        className={`inline-flex h-3.5 w-3.5 shrink-0 rounded border ${
          checked ? "border-optimal bg-optimal" : "border-ash"
        }`}
      />
      {label}
    </button>
  );
}

function FieldHelp({ children }: { children: ReactNode }) {
  return (
    <details className="mt-2 rounded-lg border border-ash/20 bg-smoke/20 px-3 py-2">
      <summary className="flex cursor-pointer list-none items-center gap-2 text-xs text-silver">
        <HelpCircle className="h-3.5 w-3.5 text-oracle" />
        Need help finding this value?
      </summary>
      <p className="mt-2 text-xs leading-relaxed text-silver">{children}</p>
    </details>
  );
}
