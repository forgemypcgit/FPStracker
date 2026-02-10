import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Cpu, MemoryStick, Search, Video, Loader2, ArrowRight } from "lucide-react";
import { useHardwareDetection } from "@/hooks/useHardwareDetection";
import { HardwareSpecs, useTrackerStore } from "@/lib/trackerStore";
import type { ReactNode } from "react";

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

  useEffect(() => {
    if (storedHardware) {
      setSpecs(storedHardware);
    }
  }, [storedHardware]);

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

  const canContinue = specs.gpu.name.trim() !== "" && specs.cpu.name.trim() !== "";

  const runDetection = async () => {
    const detected = await detectHardware();
    setSpecs(detected);
  };

  // SVG gauge constants
  const radius = 40;
  const circumference = 2 * Math.PI * radius;
  const strokeOffset = circumference - (specs.confidence * circumference);

  return (
    <div className="page-wrap animate-soft-slide">
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
              Auto-detect first, then correct any fields if needed.
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

      {/* Spec cards */}
      <section className="stagger-children mt-8 grid gap-4 lg:grid-cols-3">
        <SpecCard
          title="GPU"
          icon={<Video className="h-5 w-5" />}
          accentColor="text-oracle"
          borderColor="border-t-oracle/40"
          isDetecting={isDetecting}
        >
          <label className="label">Model</label>
          <input
            className="input-base"
            placeholder="e.g. GeForce RTX 4070"
            value={specs.gpu.name}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, gpu: { ...c.gpu, name: e.target.value } }))
            }
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
          />
          {specs.gpu.vram_mb > 0 && (
            <p className="mt-1.5 text-xs text-silver/60">
              {(specs.gpu.vram_mb / 1024).toFixed(1)} GB
            </p>
          )}
        </SpecCard>

        <SpecCard
          title="CPU"
          icon={<Cpu className="h-5 w-5" />}
          accentColor="text-optimal"
          borderColor="border-t-optimal/40"
          isDetecting={isDetecting}
        >
          <label className="label">Model</label>
          <input
            className="input-base"
            placeholder="e.g. Ryzen 7 7800X3D"
            value={specs.cpu.name}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, cpu: { ...c.cpu, name: e.target.value } }))
            }
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
              />
            </div>
          </div>
        </SpecCard>

        <SpecCard
          title="RAM"
          icon={<MemoryStick className="h-5 w-5" />}
          accentColor="text-caution"
          borderColor="border-t-caution/40"
          isDetecting={isDetecting}
        >
          <label className="label">Total Memory (MB)</label>
          <input
            className="input-base font-mono"
            type="number"
            min={0}
            value={specs.ram.total_mb || ""}
            onChange={(e) =>
              setSpecs((c) => ({ ...c, ram: { ...c.ram, total_mb: Number(e.target.value) || 0 } }))
            }
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
          />
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
            navigate("/contribute/game");
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
