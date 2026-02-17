import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AlertTriangle, ChevronLeft, ShieldAlert, Settings, Activity } from 'lucide-react';
import type { BenchmarkData } from '@/lib/trackerStore';
import { useTrackerStore } from '@/lib/trackerStore';
import { RESOLUTION_OPTIONS } from '@/lib/resolution';

const initialData: BenchmarkData = {
  resolution: '1440p',
  preset: 'High',
  ray_tracing: false,
  upscaling: 'None',
  capture_method: 'in_game_counter',
  fps_avg: 0,
  fps_1_low: undefined,
  fps_01_low: undefined,
  test_location: '',
  anti_cheat_acknowledged: false,
  anti_cheat_strict_acknowledged: false,
};

export default function BenchmarkPage() {
  const navigate = useNavigate();
  const { selectedGame, setBenchmark } = useTrackerStore();
  const [data, setData] = useState<BenchmarkData>(initialData);
  const [submitted, setSubmitted] = useState(false);
  const [strictPhrase, setStrictPhrase] = useState('');

  if (!selectedGame) {
    navigate('/contribute/game');
    return null;
  }

  const supportsRt = Boolean(selectedGame.supports_rt);
  const supportsDlss = Boolean(selectedGame.supports_dlss);
  const supportsFsr = Boolean(selectedGame.supports_fsr);
  const antiCheatRisk = selectedGame.anti_cheat_risk ?? 'medium';

  const upscalingOptions = useMemo(() => {
    const options = ['None'];
    if (supportsDlss) options.push('DLSS Quality', 'DLSS Balanced', 'DLSS Performance');
    if (supportsFsr) options.push('FSR Quality', 'FSR Balanced', 'FSR Performance');
    return options;
  }, [supportsDlss, supportsFsr]);

  useEffect(() => {
    setData((c) => ({
      ...c,
      ray_tracing: supportsRt ? c.ray_tracing : false,
      upscaling: upscalingOptions.includes(c.upscaling) ? c.upscaling : 'None',
      capture_method:
        antiCheatRisk === 'high' && c.capture_method === 'external_tool'
          ? 'in_game_counter'
          : c.capture_method,
    }));
  }, [supportsRt, upscalingOptions, antiCheatRisk]);

  const validation = useMemo(() => {
    const issues: string[] = [];
    if (!Number.isFinite(data.fps_avg)) issues.push('Average FPS must be a valid number.');
    if (data.fps_avg <= 0) issues.push('Average FPS is required.');
    if (data.fps_avg > 500) issues.push('Average FPS is above the current API limit (max 500).');
    if (data.fps_1_low !== undefined) {
      if (!Number.isFinite(data.fps_1_low)) issues.push('1% low must be a valid number.');
      if (data.fps_1_low <= 0) issues.push('1% low must be at least 1.');
      if (data.fps_1_low > 500) issues.push('1% low FPS is above the current API limit (max 500).');
      if (Number.isFinite(data.fps_avg) && data.fps_1_low > data.fps_avg)
        issues.push('1% low should be below average.');
    }
    if (data.fps_01_low !== undefined) {
      if (!Number.isFinite(data.fps_01_low)) issues.push('0.1% low must be a valid number.');
      if (data.fps_01_low <= 0) issues.push('0.1% low must be at least 1.');
      if (data.fps_01_low > 500) issues.push('0.1% low FPS is above the current API limit (max 500).');
      if (Number.isFinite(data.fps_avg) && data.fps_01_low > data.fps_avg)
        issues.push('0.1% low should be below average.');
      if (data.fps_1_low !== undefined && data.fps_01_low > data.fps_1_low)
        issues.push('0.1% low should not be higher than 1% low.');
    }
    if (antiCheatRisk === 'medium' && !data.anti_cheat_acknowledged)
      issues.push('Acknowledge anti-cheat safety.');
    if (antiCheatRisk === 'high') {
      if (!data.anti_cheat_acknowledged) issues.push('Strict anti-cheat consent required.');
      if (!data.anti_cheat_strict_acknowledged) issues.push('Confirm manual-safe capture.');
      if (strictPhrase.trim().toUpperCase() !== 'SAFE MODE') issues.push('Type SAFE MODE to confirm.');
      if (data.capture_method === 'external_tool') issues.push('External capture blocked for strict AC.');
    }
    return issues;
  }, [data, antiCheatRisk, strictPhrase]);

  const isValid = validation.length === 0;

  // FPS quality indicator
  const fpsQuality = useMemo(() => {
    if (data.fps_avg <= 0) return null;
    if (data.fps_avg >= 144) return { label: 'Excellent', color: 'text-optimal', bar: 'bg-optimal' };
    if (data.fps_avg >= 60) return { label: 'Smooth', color: 'text-oracle', bar: 'bg-oracle' };
    if (data.fps_avg >= 30) return { label: 'Playable', color: 'text-caution', bar: 'bg-caution' };
    return { label: 'Low', color: 'text-critical', bar: 'bg-critical' };
  }, [data.fps_avg]);

  return (
    <div className="animate-soft-slide space-y-6">
      {/* Header */}
      <div className="flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-oracle/10">
          <Activity className="h-5 w-5 text-oracle" />
        </div>
        <div>
          <h2 className="text-2xl font-semibold text-white">Benchmark Results</h2>
          <p className="mt-1 text-sm text-silver">
            <span className="font-medium text-pearl">{selectedGame.name}</span>
            <span className="mx-2 text-ash/60">/</span>
            <span className="text-xs">{selectedGame.difficulty ?? 'medium'} load</span>
            {selectedGame.has_benchmark && (
              <>
                <span className="mx-2 text-ash/60">/</span>
                <span className="text-xs text-optimal">built-in bench</span>
              </>
            )}
          </p>
        </div>
      </div>

      {/* Anti-cheat warning */}
      {antiCheatRisk !== 'low' && (
        <div
          className={`flex items-start gap-3 rounded-xl border px-4 py-3.5 text-sm ${
            antiCheatRisk === 'high'
              ? 'border-critical/20 bg-critical/[0.06] text-critical'
              : 'border-caution/20 bg-caution/[0.06] text-caution'
          }`}
        >
          <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0" />
          <span>
            {antiCheatRisk === 'high'
              ? 'Strict anti-cheat: use in-game counters only. No injection or hooking tools.'
              : 'Moderate anti-cheat: prefer built-in counters and benchmark scenes.'}
          </span>
        </div>
      )}

      {/* Settings */}
      <div className="panel-glow space-y-5">
        <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-[0.14em] text-silver/70">
          <Settings className="h-3.5 w-3.5" />
          Settings
        </div>

        {selectedGame.benchmark_notes && (
          <div className="rounded-lg border border-oracle/15 bg-oracle/[0.04] px-4 py-3 text-sm text-silver">
            <span className="text-[10px] font-bold uppercase tracking-widest text-oracle">
              Recommended scene
            </span>
            <p className="mt-1">{selectedGame.benchmark_notes}</p>
          </div>
        )}

        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <label className="label">Resolution</label>
            <select
              className="input-base"
              value={data.resolution}
              onChange={(e) => setData((v) => ({ ...v, resolution: e.target.value }))}
            >
              {RESOLUTION_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </div>
          <div>
            <label className="label">Preset</label>
            <select
              className="input-base"
              value={data.preset}
              onChange={(e) => setData((v) => ({ ...v, preset: e.target.value }))}
            >
              <option>Low</option>
              <option>Medium</option>
              <option>High</option>
              <option>Ultra</option>
              <option>Custom</option>
            </select>
          </div>
        </div>

        {/* FPS inputs with quality bar */}
        <div>
          <div className="flex items-center gap-2 text-xs font-bold uppercase tracking-[0.14em] text-silver/70">
            <Activity className="h-3.5 w-3.5" />
            Performance
          </div>
          <div className="mt-3 grid gap-4 sm:grid-cols-3">
            <div>
              <label className="label">Average FPS</label>
              <input
                className="input-base font-mono text-lg"
                type="number"
                min={1}
                max={500}
                placeholder="96"
                value={data.fps_avg || ''}
                onChange={(e) => setData((v) => ({ ...v, fps_avg: Number(e.target.value) || 0 }))}
              />
              {fpsQuality && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-smoke/60">
                    <div
                      className={`h-full rounded-full transition-all duration-500 ${fpsQuality.bar}`}
                      style={{ width: `${Math.min(100, (data.fps_avg / 240) * 100)}%` }}
                    />
                  </div>
                  <span className={`text-[11px] font-semibold ${fpsQuality.color}`}>
                    {fpsQuality.label}
                  </span>
                </div>
              )}
            </div>
            <div>
              <label className="label">1% Low FPS</label>
              <input
                className="input-base font-mono text-lg"
                type="number"
                min={1}
                max={500}
                placeholder="Optional"
                value={data.fps_1_low || ''}
                onChange={(e) =>
                  setData((v) => ({ ...v, fps_1_low: Number(e.target.value) || undefined }))
                }
              />
            </div>
            <div>
              <label className="label">0.1% Low FPS</label>
              <input
                className="input-base font-mono text-lg"
                type="number"
                min={1}
                max={500}
                placeholder="Optional"
                value={data.fps_01_low || ''}
                onChange={(e) =>
                  setData((v) => ({ ...v, fps_01_low: Number(e.target.value) || undefined }))
                }
              />
            </div>
          </div>
        </div>

        <div className="divider" />

        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <label className="label">Capture Method</label>
            <select
              className="input-base"
              value={data.capture_method}
              onChange={(e) =>
                setData((v) => ({
                  ...v,
                  capture_method: e.target.value as BenchmarkData['capture_method'],
                }))
              }
            >
              <option value="in_game_counter">In-game counter</option>
              <option value="built_in_benchmark">Built-in benchmark</option>
              {antiCheatRisk !== 'high' && (
                <option value="external_tool">External tool</option>
              )}
            </select>
            {antiCheatRisk === 'high' && (
              <p className="mt-1 text-[11px] text-critical">
                External capture disabled for strict AC titles.
              </p>
            )}
          </div>
          <div>
            <label className="label">Upscaling</label>
            <select
              className="input-base"
              value={data.upscaling}
              disabled={upscalingOptions.length <= 1}
              onChange={(e) => setData((v) => ({ ...v, upscaling: e.target.value }))}
            >
              {upscalingOptions.map((o) => (
                <option key={o}>{o}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <label className="label">Ray Tracing</label>
            <button
              type="button"
              disabled={!supportsRt}
              className={`input-base text-left ${
                supportsRt && data.ray_tracing ? 'border-oracle/40 text-oracle' : ''
              } ${!supportsRt ? 'opacity-50' : 'cursor-pointer'}`}
              onClick={() =>
                supportsRt && setData((v) => ({ ...v, ray_tracing: !v.ray_tracing }))
              }
            >
              {supportsRt ? (data.ray_tracing ? 'Enabled' : 'Disabled') : 'Not supported'}
            </button>
          </div>
          <div>
            <label className="label">Scene Notes</label>
            <input
              className="input-base"
              placeholder="Optional"
              value={data.test_location || ''}
              onChange={(e) => setData((v) => ({ ...v, test_location: e.target.value }))}
            />
          </div>
        </div>
      </div>

      {/* Anti-cheat consent */}
      {antiCheatRisk === 'medium' && (
        <div className="rounded-xl border border-caution/20 bg-caution/[0.04] p-4">
          <p className="mb-3 text-sm font-medium text-caution">Anti-cheat acknowledgment</p>
          <ConsentButton
            checked={data.anti_cheat_acknowledged}
            onToggle={() =>
              setData((v) => ({ ...v, anti_cheat_acknowledged: !v.anti_cheat_acknowledged }))
            }
            label="This benchmark follows the game's anti-cheat policy."
            tone="caution"
          />
        </div>
      )}

      {antiCheatRisk === 'high' && (
        <div className="space-y-2.5 rounded-xl border border-critical/20 bg-critical/[0.04] p-4">
          <p className="mb-1 text-sm font-medium text-critical">Strict anti-cheat consent</p>
          <ConsentButton
            checked={data.anti_cheat_acknowledged}
            onToggle={() =>
              setData((v) => ({ ...v, anti_cheat_acknowledged: !v.anti_cheat_acknowledged }))
            }
            label="I used only in-game counter or built-in benchmark."
            tone="critical"
          />
          <ConsentButton
            checked={data.anti_cheat_strict_acknowledged ?? false}
            onToggle={() =>
              setData((v) => ({
                ...v,
                anti_cheat_strict_acknowledged: !v.anti_cheat_strict_acknowledged,
              }))
            }
            label="I understand violating anti-cheat policy can risk penalties."
            tone="critical"
          />
          <div className="mt-2">
            <label className="label text-critical">Type SAFE MODE to confirm</label>
            <input
              className="input-base border-critical/30 bg-critical/5 font-mono uppercase"
              placeholder="SAFE MODE"
              value={strictPhrase}
              onChange={(e) => setStrictPhrase(e.target.value)}
            />
          </div>
        </div>
      )}

      {/* Validation */}
      {submitted && !isValid && (
        <div className="rounded-xl border border-critical/20 bg-critical/[0.06] p-4">
          <p className="mb-2 flex items-center gap-2 text-sm font-medium text-critical">
            <AlertTriangle className="h-4 w-4" /> Please fix:
          </p>
          <ul className="list-disc space-y-1 pl-5 text-sm text-silver">
            {validation.map((v) => (
              <li key={v}>{v}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Navigation */}
      <div className="flex items-center justify-between pt-2">
        <button
          type="button"
          className="btn-secondary"
          onClick={() => navigate('/contribute/game')}
        >
          <ChevronLeft className="h-4 w-4" /> Back
        </button>
        <button
          type="button"
          className="btn-primary"
          onClick={() => {
            setSubmitted(true);
            if (!isValid) return;
            setBenchmark(data);
            navigate('/contribute/review');
          }}
        >
          Review
        </button>
      </div>
    </div>
  );
}

interface ConsentButtonProps {
  checked: boolean;
  onToggle: () => void;
  label: string;
  tone: 'caution' | 'critical';
}

function ConsentButton({ checked, onToggle, label, tone }: ConsentButtonProps) {
  const activeClasses =
    tone === 'critical' ? 'border-critical/40 bg-critical/10' : 'border-caution/40 bg-caution/10';
  const checkClasses =
    tone === 'critical' ? 'border-critical bg-critical' : 'border-caution bg-caution';

  return (
    <button
      type="button"
      onClick={onToggle}
      className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-all duration-200 ${
        checked ? activeClasses : 'border-ash/30 bg-smoke/30 hover:border-ash/50'
      }`}
    >
      <span
        className={`flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-all duration-200 ${
          checked ? checkClasses : 'border-ash'
        }`}
      >
        {checked && <span className="block h-2 w-2 rounded-sm bg-void" />}
      </span>
      <span className="text-sm text-pearl">{label}</span>
    </button>
  );
}
