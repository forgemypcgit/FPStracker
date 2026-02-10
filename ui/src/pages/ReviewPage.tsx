import { useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { AlertTriangle, ChevronLeft, Send, Gamepad2, Activity, Cpu } from 'lucide-react';
import { useTrackerStore } from '@/lib/trackerStore';
import type { ReactNode } from 'react';
import { formatResolution } from '@/lib/resolution';

interface SubmitResponse {
  submission_id: string;
  status: string;
  message: string;
}

export default function ReviewPage() {
  const navigate = useNavigate();
  const { hardware, selectedGame, benchmark } = useTrackerStore();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [strictFinalConsent, setStrictFinalConsent] = useState(false);

  if (!hardware || !selectedGame || !benchmark) {
    navigate('/contribute/game');
    return null;
  }

  const antiCheatRisk = selectedGame.anti_cheat_risk ?? 'medium';
  const requiresStrictFinalConsent = antiCheatRisk === 'high';

  const payload = useMemo(() => {
    const inferGpuVendor = (name: string): 'Nvidia' | 'Amd' | 'Intel' | 'Unknown' => {
      const l = name.toLowerCase();
      if (l.includes('nvidia') || l.includes('geforce') || l.includes('rtx')) return 'Nvidia';
      if (l.includes('amd') || l.includes('radeon') || l.includes('rx ')) return 'Amd';
      if (l.includes('intel') || l.includes('arc')) return 'Intel';
      return 'Unknown';
    };

    const inferCpuVendor = (name: string): string => {
      const l = name.toLowerCase();
      if (l.includes('intel')) return 'Intel';
      if (l.includes('amd') || l.includes('ryzen')) return 'AMD';
      return 'Unknown';
    };

    return {
      id: crypto.randomUUID(),
      timestamp: new Date().toISOString(),
      system_info: {
        gpu: {
          name: hardware.gpu.name,
          vendor: inferGpuVendor(hardware.gpu.name),
          vram_mb: hardware.gpu.vram_mb || null,
          driver_version: hardware.gpu.driver_version ?? null,
          pci_id: null,
          gpu_clock_mhz: hardware.gpu.clock_mhz ?? null,
          memory_clock_mhz: null,
          temperature_c: null,
          utilization_percent: null,
        },
        cpu: {
          name: hardware.cpu.name,
          cores: hardware.cpu.cores,
          threads: hardware.cpu.threads,
          frequency_mhz: hardware.cpu.frequency_mhz ?? null,
          max_frequency_mhz: null,
          architecture: null,
          vendor: inferCpuVendor(hardware.cpu.name),
        },
        ram: {
          installed_mb: hardware.ram.total_mb || null,
          usable_mb: hardware.ram.total_mb || 0,
          speed_mhz: hardware.ram.speed_mhz ?? null,
          ram_type: hardware.ram.type ?? null,
          stick_count: null,
          model: null,
        },
        os: navigator.platform || 'Unknown',
        os_version: null,
      },
      game: selectedGame.name,
      resolution: benchmark.resolution,
      preset: benchmark.preset,
      avg_fps: benchmark.fps_avg,
      fps_1_low: benchmark.fps_1_low ?? null,
      fps_01_low: benchmark.fps_01_low ?? null,
      ray_tracing: benchmark.ray_tracing,
      upscaling: benchmark.upscaling === 'None' ? null : benchmark.upscaling,
      frame_gen: null,
      sample_count: null,
      duration_secs: null,
      notes: benchmark.test_location?.trim() ? benchmark.test_location.trim() : null,
      capture_method: benchmark.capture_method,
      anti_cheat_acknowledged: benchmark.anti_cheat_acknowledged,
      anti_cheat_strict_acknowledged: benchmark.anti_cheat_strict_acknowledged ?? false,
    };
  }, [benchmark, hardware, selectedGame.name]);

  const submit = async () => {
    if (requiresStrictFinalConsent && !strictFinalConsent) {
      setSubmitError('Final strict anti-cheat consent is required.');
      return;
    }

    setSubmitError(null);
    setIsSubmitting(true);
    try {
      const response = await fetch('/api/benchmark/submit', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!response.ok) throw new Error(`Submission failed (${response.status})`);

      const result = (await response.json()) as SubmitResponse;
      navigate('/success', {
        state: {
          submissionId: result.submission_id,
          status: result.status,
          message: result.message,
        },
      });
    } catch (err) {
      setSubmitError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setIsSubmitting(false);
    }
  };

  const captureLabel =
    benchmark.capture_method === 'in_game_counter'
      ? 'In-game counter'
      : benchmark.capture_method === 'built_in_benchmark'
      ? 'Built-in benchmark'
      : 'External tool';

  return (
    <div className="animate-soft-slide space-y-6">
      <div>
        <h2 className="text-2xl font-semibold text-white">Review</h2>
        <p className="mt-1.5 text-sm text-silver">Confirm everything, then submit.</p>
      </div>

      <SummarySection
        title="Game & Settings"
        icon={<Gamepad2 className="h-3.5 w-3.5 text-oracle" />}
      >
        <Row label="Game" value={selectedGame.name} />
        <Row label="Resolution" value={formatResolution(benchmark.resolution)} />
        <Row label="Preset" value={benchmark.preset} />
        <Row label="Ray Tracing" value={benchmark.ray_tracing ? 'On' : 'Off'} />
        <Row label="Upscaling" value={benchmark.upscaling || 'None'} />
        <Row label="Capture" value={captureLabel} />
      </SummarySection>

      <SummarySection
        title="Performance"
        icon={<Activity className="h-3.5 w-3.5 text-optimal" />}
      >
        <Row label="Avg FPS" value={String(benchmark.fps_avg)} mono highlight />
        <Row label="1% Low" value={benchmark.fps_1_low ? String(benchmark.fps_1_low) : '\u2014'} mono />
        <Row label="0.1% Low" value={benchmark.fps_01_low ? String(benchmark.fps_01_low) : '\u2014'} mono />
        {benchmark.test_location && <Row label="Notes" value={benchmark.test_location} />}
      </SummarySection>

      <SummarySection
        title="Hardware"
        icon={<Cpu className="h-3.5 w-3.5 text-caution" />}
      >
        <Row label="GPU" value={hardware.gpu.name} />
        <Row
          label="CPU"
          value={`${hardware.cpu.name} (${hardware.cpu.cores}C/${hardware.cpu.threads}T)`}
        />
        <Row label="RAM" value={`${(hardware.ram.total_mb / 1024).toFixed(0)} GB`} />
      </SummarySection>

      {submitError && (
        <div className="flex items-start gap-2 rounded-xl border border-critical/20 bg-critical/[0.06] px-4 py-3 text-sm text-critical">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          {submitError}
        </div>
      )}

      {requiresStrictFinalConsent && (
        <button
          type="button"
          onClick={() => setStrictFinalConsent((v) => !v)}
          className={`flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all duration-200 ${
            strictFinalConsent
              ? 'border-critical/40 bg-critical/10'
              : 'border-ash/30 bg-smoke/30 hover:border-ash/50'
          }`}
        >
          <span
            className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-all duration-200 ${
              strictFinalConsent ? 'border-critical bg-critical' : 'border-ash'
            }`}
          >
            {strictFinalConsent && <span className="block h-2 w-2 rounded-sm bg-void" />}
          </span>
          <span className="text-sm text-pearl">
            Final confirmation: captured with safe in-game or built-in methods only.
          </span>
        </button>
      )}

      <div className="flex items-center justify-between pt-2">
        <Link to="/contribute/benchmark" className="btn-secondary">
          <ChevronLeft className="h-4 w-4" /> Back
        </Link>
        <button
          type="button"
          className="btn-primary group"
          disabled={isSubmitting}
          onClick={submit}
        >
          {isSubmitting ? 'Submitting...' : 'Submit'}
          {!isSubmitting && <Send className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />}
        </button>
      </div>
    </div>
  );
}

function SummarySection({
  title,
  icon,
  children,
}: {
  title: string;
  icon?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="overflow-hidden rounded-2xl border border-ash/30 bg-obsidian/60 backdrop-blur-sm"
         style={{ boxShadow: 'inset 0 1px 0 0 rgba(255,255,255,0.04)' }}
    >
      <div className="flex items-center gap-2 px-5 py-3">
        {icon}
        <h3 className="text-xs font-bold uppercase tracking-[0.14em] text-silver">{title}</h3>
      </div>
      <div className="divide-y divide-ash/15">{children}</div>
    </div>
  );
}

interface RowProps {
  label: string;
  value: string;
  mono?: boolean;
  highlight?: boolean;
}

function Row({ label, value, mono, highlight }: RowProps) {
  return (
    <div className="flex items-center justify-between gap-4 px-5 py-2.5">
      <span className="text-sm text-silver">{label}</span>
      <span
        className={`text-right text-sm ${mono ? 'font-mono' : ''} ${
          highlight ? 'text-lg font-bold text-oracle' : 'text-white'
        }`}
      >
        {value}
      </span>
    </div>
  );
}
