import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Activity, Check, Loader2, ArrowRight, ChevronDown } from 'lucide-react';
import { useTrackerStore } from '@/lib/trackerStore';

type Profile = 'quick' | 'standard' | 'extended';

type SyntheticInstallCommandResponse = {
  ok: boolean;
  command?: string | null;
  tools_missing: string[];
  message?: string | null;
};

type SyntheticApiResponse = {
  synthetic_suite_version?: string | null;
  cpu_score?: number | null;
  cpu_score_source?: string | null;
  gpu_score?: number | null;
  gpu_score_source?: string | null;
  ram_score?: number | null;
  ram_score_source?: string | null;
  disk_score?: number | null;
  disk_score_source?: string | null;
  duration_secs?: number | null;
  winsat_note?: string | null;
  cpu_7z_single_mips?: number | null;
  cpu_7z_multi_mips?: number | null;
  diskspd_read_mb_s?: number | null;
  diskspd_write_mb_s?: number | null;
  blender_cpu_render_ms?: number | null;
  blender_cpu_render_settings?: string | null;
  sysbench_cpu_1t_events_s?: number | null;
  sysbench_cpu_mt_events_s?: number | null;
  sysbench_memory_mib_s?: number | null;
  fio_seq_read_mib_s?: number | null;
  fio_seq_write_mib_s?: number | null;
  fio_randread_iops?: number | null;
  fio_randwrite_iops?: number | null;
};

type SyntheticProgressUpdate = {
  completed_steps: number;
  total_steps: number;
  status: string;
};

export default function SyntheticPage() {
  const navigate = useNavigate();
  const { hardware, syntheticBaseline, setSyntheticBaseline, markSyntheticStepSeen } =
    useTrackerStore();
  const [running, setRunning] = useState(false);
  const [profile, setProfile] = useState<Profile>('standard');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [requiresAdmin, setRequiresAdmin] = useState(false);
  const [installCmd, setInstallCmd] = useState<string | null>(null);
  const [missingTools, setMissingTools] = useState<string[]>([]);
  const [runStartedAt, setRunStartedAt] = useState<number | null>(null);
  const [elapsedSecs, setElapsedSecs] = useState(0);
  const [lastRunDurationSecs, setLastRunDurationSecs] = useState<number | null>(null);
  const [hasRunAttempted, setHasRunAttempted] = useState(false);
  const [progress, setProgress] = useState<SyntheticProgressUpdate | null>(null);
  const [progressLog, setProgressLog] = useState<string[]>([]);
  const streamRef = useRef<EventSource | null>(null);

  if (!hardware) {
    navigate('/contribute/detect');
    return null;
  }

  useEffect(() => {
    // Mark this step as visited as soon as the user lands here so deep-links to later steps
    // don't bounce them back unexpectedly.
    markSyntheticStepSeen();
  }, [markSyntheticStepSeen]);

  useEffect(() => {
    if (hardware.os_family !== 'linux' && hardware.os_family !== 'macos') {
      return;
    }
    let cancelled = false;
    fetch('/api/deps/synthetic/install-command')
      .then((r) => r.json().catch(() => null))
      .then((payload: SyntheticInstallCommandResponse | null) => {
        if (cancelled || !payload?.ok) return;
        setInstallCmd(payload.command ? String(payload.command) : null);
        setMissingTools(Array.isArray(payload.tools_missing) ? payload.tools_missing : []);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [hardware.os_family]);

  const metrics = useMemo(
    () => [
      { key: 'cpu', label: 'CPU', value: syntheticBaseline?.synthetic_cpu_score },
      { key: 'gpu', label: 'GPU', value: syntheticBaseline?.synthetic_gpu_score },
      { key: 'ram', label: 'RAM', value: syntheticBaseline?.synthetic_ram_score },
      { key: 'disk', label: 'SSD', value: syntheticBaseline?.synthetic_disk_score },
    ],
    [syntheticBaseline]
  );

  const capturedCount = metrics.filter((m) => m.value !== undefined).length;
  const expectedDurationSecs = profile === 'quick' ? 30 : profile === 'extended' ? 300 : 120;
  const runningStages = useMemo(() => {
    if (hardware.os_family === 'linux') {
      return [
        'CPU benchmark',
        'RAM benchmark',
        'Disk benchmark',
        'GPU benchmark (optional)',
        'Finalizing results',
      ];
    }
    if (hardware.os_family === 'windows') {
      return [
        'Windows synthetic collection',
        'CPU/RAM/SSD checks',
        'GPU checks',
        'Finalizing results',
      ];
    }
    return ['CPU benchmark', 'GPU benchmark', 'Finalizing results'];
  }, [hardware.os_family]);
  const stageIndex = Math.min(
    runningStages.length - 1,
    Math.floor((Math.max(elapsedSecs, 0) / Math.max(expectedDurationSecs, 1)) * runningStages.length)
  );
  const stageLabel = progress?.status ?? runningStages[Math.max(stageIndex, 0)] ?? 'Running benchmarks';
  const progressPercent = progress
    ? Math.min(99, Math.round((progress.completed_steps / Math.max(progress.total_steps, 1)) * 100))
    : Math.min(99, Math.round((Math.max(elapsedSecs, 0) / Math.max(expectedDurationSecs, 1)) * 100));
  const stageStates = runningStages.map((label, idx) => {
    if (running) {
      const effectiveIndex = progress
        ? progress.completed_steps >= progress.total_steps
          ? runningStages.length - 1
          : Math.min(runningStages.length - 1, Math.max(progress.completed_steps - 1, 0))
        : stageIndex;
      if (idx < effectiveIndex) return { label, state: 'done' as const };
      if (idx === effectiveIndex) return { label, state: 'running' as const };
      return { label, state: 'pending' as const };
    }
    if (error && hasRunAttempted) {
      if (idx < stageIndex) return { label, state: 'done' as const };
      if (idx === stageIndex) return { label, state: 'failed' as const };
      return { label, state: 'pending' as const };
    }
    if (!error && lastRunDurationSecs !== null) {
      return { label, state: 'done' as const };
    }
    return { label, state: 'pending' as const };
  });

  useEffect(() => {
    return () => {
      if (streamRef.current) {
        streamRef.current.close();
        streamRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!running || runStartedAt == null) {
      return;
    }
    const timer = window.setInterval(() => {
      const next = Math.max(0, (Date.now() - runStartedAt) / 1000);
      setElapsedSecs(next);
    }, 200);
    return () => window.clearInterval(timer);
  }, [running, runStartedAt]);

  const finalizeRun = (startedAtMs: number, payload: SyntheticApiResponse) => {
    const measuredDuration =
      typeof payload.duration_secs === 'number' && Number.isFinite(payload.duration_secs)
        ? payload.duration_secs
        : Math.max(0, (Date.now() - startedAtMs) / 1000);
    setLastRunDurationSecs(measuredDuration);
    setSyntheticBaseline({
      synthetic_cpu_score: payload.cpu_score ?? undefined,
      synthetic_cpu_source: payload.cpu_score_source ?? undefined,
      synthetic_gpu_score: payload.gpu_score ?? undefined,
      synthetic_gpu_source: payload.gpu_score_source ?? undefined,
      synthetic_ram_score: payload.ram_score ?? undefined,
      synthetic_ram_source: payload.ram_score_source ?? undefined,
      synthetic_disk_score: payload.disk_score ?? undefined,
      synthetic_disk_source: payload.disk_score_source ?? undefined,
      synthetic_profile: profile,
      synthetic_suite_version: payload.synthetic_suite_version ?? undefined,
      // Keep the full raw response for audit/debug and downstream validation.
      synthetic_extended: (payload as unknown as Record<string, unknown>) ?? undefined,
    });
  };

  const runSynthetic = () => {
    const startedAt = Date.now();
    setRunning(true);
    setError(null);
    setRequiresAdmin(false);
    setHasRunAttempted(true);
    setRunStartedAt(startedAt);
    setElapsedSecs(0);
    setLastRunDurationSecs(null);
    setProgress(null);
    setProgressLog([]);

    if (streamRef.current) {
      streamRef.current.close();
      streamRef.current = null;
    }

    const streamUrl = `/api/benchmark/synthetic/stream?profile=${encodeURIComponent(profile)}`;
    const es = new EventSource(streamUrl);
    streamRef.current = es;
    let fallbackTriggered = false;

    es.addEventListener('start', () => {
      setProgressLog((prev) => prev.length === 0 ? ['Starting synthetic benchmarks...'] : prev);
    });

    es.addEventListener('progress', (evt) => {
      try {
        const next = JSON.parse((evt as MessageEvent).data) as SyntheticProgressUpdate;
        if (!next || typeof next.status !== 'string') return;
        setProgress(next);
        setProgressLog((prev) => {
          const label = next.status.trim();
          if (!label) return prev;
          if (prev.length > 0 && prev[prev.length - 1] === label) return prev;
          const nextLog = [...prev, label];
          return nextLog.slice(-8);
        });
      } catch {
        // Ignore malformed events.
      }
    });

    es.addEventListener('result', (evt) => {
      try {
        const payload = JSON.parse((evt as MessageEvent).data) as SyntheticApiResponse;
        finalizeRun(startedAt, payload);
      } catch {
        setError('Synthetic run returned an invalid response.');
      } finally {
        fallbackTriggered = true;
        setRunning(false);
        setRunStartedAt(null);
        setProgress(null);
        es.close();
        streamRef.current = null;
      }
    });

    es.addEventListener('bench_error', (evt) => {
      try {
        const payload = JSON.parse((evt as MessageEvent).data) as { error?: string; requires_admin?: boolean };
        setRequiresAdmin(Boolean(payload?.requires_admin));
        setError(payload?.error ? String(payload.error) : 'Synthetic run failed.');
      } catch {
        setError('Synthetic run failed.');
      } finally {
        fallbackTriggered = true;
        setRunning(false);
        setRunStartedAt(null);
        setProgress(null);
        es.close();
        streamRef.current = null;
      }
    });

    es.onerror = async () => {
      if (fallbackTriggered) return;
      fallbackTriggered = true;
      // If the SSE transport fails (proxies/extensions), fall back to the standard POST endpoint.
      es.close();
      if (streamRef.current === es) {
        streamRef.current = null;
      }

      try {
        const resp = await fetch(
          `/api/benchmark/synthetic/run?profile=${encodeURIComponent(profile)}`,
          { method: 'POST' }
        );
        const payload = (await resp.json().catch(() => null)) as
          | SyntheticApiResponse
          | { error?: string; requires_admin?: boolean }
          | null;

        if (!resp.ok) {
          setRequiresAdmin(Boolean((payload as any)?.requires_admin));
          setError(
            (payload as any)?.error
              ? String((payload as any).error)
              : `Synthetic run failed (${resp.status}).`
          );
          return;
        }

        finalizeRun(startedAt, payload as SyntheticApiResponse);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(`Synthetic run failed: ${msg}`);
      } finally {
        setRunning(false);
        setRunStartedAt(null);
        setProgress(null);
      }
    };
  };

  const setMetric = (key: 'cpu' | 'gpu' | 'ram' | 'disk', value: number | undefined) => {
    setSyntheticBaseline({
      synthetic_cpu_score:
        key === 'cpu' ? value : syntheticBaseline?.synthetic_cpu_score,
      synthetic_cpu_source:
        key === 'cpu' ? undefined : syntheticBaseline?.synthetic_cpu_source,
      synthetic_gpu_score:
        key === 'gpu' ? value : syntheticBaseline?.synthetic_gpu_score,
      synthetic_gpu_source:
        key === 'gpu' ? undefined : syntheticBaseline?.synthetic_gpu_source,
      synthetic_ram_score:
        key === 'ram' ? value : syntheticBaseline?.synthetic_ram_score,
      synthetic_ram_source:
        key === 'ram' ? undefined : syntheticBaseline?.synthetic_ram_source,
      synthetic_disk_score:
        key === 'disk' ? value : syntheticBaseline?.synthetic_disk_score,
      synthetic_disk_source:
        key === 'disk' ? undefined : syntheticBaseline?.synthetic_disk_source,
      synthetic_profile: syntheticBaseline?.synthetic_profile ?? profile,
      synthetic_suite_version: syntheticBaseline?.synthetic_suite_version,
      synthetic_extended: syntheticBaseline?.synthetic_extended,
    });
  };

  return (
    <div className="animate-soft-slide">
      <div className="mb-6 flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-oracle/10">
          <Activity className="h-5 w-5 text-oracle" />
        </div>
        <div className="min-w-0">
          <h2 className="text-2xl font-semibold text-white">Synthetic baseline</h2>
          <p className="mt-1 text-sm text-silver">
            Optional. Runs a local system check and fills component scores when available.
          </p>
        </div>
      </div>

      <div className="panel-glow">
        <div className="mb-5 flex items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <div className="relative flex h-12 w-12 items-center justify-center">
              {capturedCount === 4 ? (
                <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-optimal/20">
                  <Check className="h-6 w-6 text-optimal" strokeWidth={3} />
                </div>
              ) : (
                <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-oracle/10 text-oracle">
                  <Activity className="h-5 w-5" />
                </div>
              )}
            </div>
            <div>
              <div className="text-xs text-silver/60">STATUS</div>
              <div className="font-semibold text-white">
                {capturedCount}/4 captured
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <select
              className="input-base h-10 py-1 text-xs font-mono uppercase tracking-[0.12em]"
              value={profile}
              disabled={running}
              onChange={(e) => setProfile(e.target.value as Profile)}
            >
              <option value="quick">Quick</option>
              <option value="standard">Standard</option>
              <option value="extended">Extended</option>
            </select>
            <button
              type="button"
              className="btn-secondary"
              onClick={runSynthetic}
              disabled={running}
            >
              {running ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              {running ? 'Running…' : '[RUN]'}
            </button>
          </div>
        </div>

        {error && (
          <div className="mb-4 rounded-xl border border-critical/20 bg-critical/[0.06] p-4">
            <p className="mb-1 text-sm font-medium text-critical">Synthetic run failed</p>
            <p className="text-sm text-silver">{error}</p>
            {requiresAdmin && (
              <p className="mt-2 text-xs text-silver">
                Tip: try running from an Administrator terminal (Windows).
              </p>
            )}
          </div>
        )}

        {running && (
          <div className="mb-4 rounded-xl border border-oracle/20 bg-oracle/[0.06] p-4">
            <div className="flex items-center justify-between gap-3">
              <p className="text-sm font-medium text-oracle">Running synthetic benchmarks...</p>
              <span className="font-mono text-xs text-silver">{elapsedSecs.toFixed(1)}s elapsed</span>
            </div>
            <p className="mt-1 text-sm text-silver">
              Current stage: <span className="text-pearl">{stageLabel}</span>
            </p>
            <div className="mt-3 h-1.5 w-full overflow-hidden rounded-full bg-smoke/70">
              <div
                className="h-full rounded-full bg-oracle transition-all duration-300"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
            <p className="mt-2 text-xs text-silver/70">
              Linux runs may take longer when optional tools are missing or fallback tests are used.
            </p>
          </div>
        )}

        {!running && !error && lastRunDurationSecs !== null && (
          <div className="mb-4 rounded-xl border border-optimal/20 bg-optimal/[0.06] p-4 text-sm text-silver">
            <span className="font-medium text-optimal">Synthetic baseline finished.</span>{' '}
            Duration: <span className="font-mono text-pearl">{lastRunDurationSecs.toFixed(1)}s</span>
          </div>
        )}

        {missingTools.length > 0 && (
          <div className="mb-4 rounded-xl border border-caution/20 bg-caution/[0.06] p-4">
            <p className="mb-1 text-sm font-medium text-caution">Optional tools missing</p>
            <p className="text-sm text-silver">
              Synthetic baseline can capture more detail if you install:{" "}
              <span className="font-mono text-xs">{missingTools.join(', ')}</span>
              .
            </p>
            {installCmd && (
              <div className="mt-3 rounded-lg border border-white/10 bg-smoke/40 p-3">
                <div className="text-[10px] font-semibold uppercase tracking-wider text-silver/60">
                  Install command
                </div>
                <pre className="mt-2 whitespace-pre-wrap break-words font-mono text-xs text-silver">
                  {installCmd}
                </pre>
                <p className="mt-2 text-xs text-silver/70">
                  Tip: you can also run <span className="font-mono">fps-tracker doctor --fix</span>{" "}
                  in a terminal.
                </p>
              </div>
            )}
          </div>
        )}

        <div className="grid gap-4 sm:grid-cols-2">
          <MetricCard
            label="CPU"
            value={syntheticBaseline?.synthetic_cpu_score}
            source={syntheticBaseline?.synthetic_cpu_source}
            onChange={(v) => setMetric('cpu', v)}
          />
          <MetricCard
            label="GPU"
            value={syntheticBaseline?.synthetic_gpu_score}
            source={syntheticBaseline?.synthetic_gpu_source}
            onChange={(v) => setMetric('gpu', v)}
            hint="GPU score may require WinSAT (Windows) or glmark2 (Linux, optional)."
          />
          <MetricCard
            label="RAM"
            value={syntheticBaseline?.synthetic_ram_score}
            source={syntheticBaseline?.synthetic_ram_source}
            onChange={(v) => setMetric('ram', v)}
          />
          <MetricCard
            label="SSD"
            value={syntheticBaseline?.synthetic_disk_score}
            source={syntheticBaseline?.synthetic_disk_source}
            onChange={(v) => setMetric('disk', v)}
          />
        </div>

        {(running || hasRunAttempted) && (
          <div className="mt-5 rounded-xl border border-ash/20 bg-smoke/20 p-4">
            <div className="mb-2 text-xs font-semibold uppercase tracking-[0.14em] text-silver/65">
              Run Progress Log
            </div>
            {progressLog.length > 0 && (
              <div className="mb-4 rounded-lg border border-white/10 bg-smoke/35 px-3 py-2">
                <div className="flex items-center justify-between gap-3">
                  <div className="text-[10px] font-semibold uppercase tracking-wider text-silver/60">
                    Live status
                  </div>
                  {progress && (
                    <div className="font-mono text-[11px] text-silver/70">
                      {Math.max(progress.completed_steps, 0)}/{Math.max(progress.total_steps, 0)}
                    </div>
                  )}
                </div>
                <pre className="mt-2 whitespace-pre-wrap break-words font-mono text-xs text-silver">
                  {progressLog.map((line) => `• ${line}`).join('\n')}
                </pre>
              </div>
            )}
            <div className="space-y-2">
              {stageStates.map((entry, idx) => (
                <div
                  key={entry.label}
                  className={`flex items-center justify-between rounded-lg border px-3 py-2 text-xs ${
                    entry.state === 'done'
                      ? 'border-optimal/25 bg-optimal/[0.06]'
                      : entry.state === 'running'
                      ? 'border-oracle/30 bg-oracle/[0.08]'
                      : entry.state === 'failed'
                      ? 'border-critical/35 bg-critical/[0.08]'
                      : 'border-ash/25 bg-smoke/35'
                  }`}
                >
                  <div className="flex items-center gap-2">
                    <span
                      className={`h-2 w-2 rounded-full ${
                        entry.state === 'done'
                          ? 'bg-optimal'
                          : entry.state === 'running'
                          ? 'bg-oracle animate-pulse'
                          : entry.state === 'failed'
                          ? 'bg-critical'
                          : 'bg-ash'
                      }`}
                    />
                    <span className={entry.state === 'pending' ? 'text-silver/70' : 'text-pearl'}>
                      {entry.label}
                    </span>
                  </div>
                  <span className="font-mono text-[11px] text-silver/75">
                    {entry.state === 'done'
                      ? 'DONE'
                      : entry.state === 'running'
                      ? 'RUNNING'
                      : entry.state === 'failed'
                      ? 'FAILED'
                      : `WAIT ${idx + 1}/${runningStages.length}`}
                  </span>
                </div>
              ))}
            </div>
            <p className="mt-3 text-xs text-silver/65">
              Profiles change precision windows and runtime. Component tests are still attempted in all
              profiles; missing values mean unavailable data, not zero.
            </p>
            {error && (
              <p className="mt-2 text-xs text-critical/90">
                Last run ended with an error. Completed stages above are best-effort UI estimates.
              </p>
            )}
          </div>
        )}

        <button
          type="button"
          onClick={() => setShowAdvanced((v) => !v)}
          className="mt-5 flex items-center gap-2 text-sm text-silver/70 hover:text-white transition"
        >
          <ChevronDown
            className={`h-4 w-4 transition-transform ${showAdvanced ? 'rotate-180' : ''}`}
          />
          [+] ADVANCED_NOTES
        </button>

        {showAdvanced && (
          <div className="mt-3 grid gap-3">
            <div className="rounded-lg border border-ash/25 bg-smoke/25 px-3 py-2 text-sm text-silver">
              Keep downloads, browser tabs, and background apps closed while running. Disk tests may
              briefly use temp storage under your OS temp directory.
            </div>
            <div className="rounded-lg border border-ash/25 bg-smoke/25 px-3 py-2 text-sm text-silver">
              If any score is missing, leave it blank. Blank means unknown, not zero.
            </div>
            {syntheticBaseline?.synthetic_extended &&
            (syntheticBaseline.synthetic_extended as any)?.winsat_note ? (
              <div className="rounded-lg border border-caution/20 bg-caution/[0.06] px-3 py-2 text-sm text-silver">
                <div className="text-xs font-semibold uppercase tracking-[0.14em] text-caution">
                  Windows note
                </div>
                <div className="mt-1 text-sm">
                  {String((syntheticBaseline.synthetic_extended as any).winsat_note)}
                </div>
              </div>
            ) : null}
          </div>
        )}
      </div>

      <div className="mt-6 flex justify-end">
        <button
          type="button"
          className="btn-primary group"
          onClick={() => {
            markSyntheticStepSeen();
            navigate('/contribute/game');
          }}
        >
          Continue
          <ArrowRight className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />
        </button>
      </div>
    </div>
  );
}

function MetricCard({
  label,
  value,
  source,
  hint,
  onChange,
}: {
  label: string;
  value: number | undefined;
  source?: string;
  hint?: string;
  onChange: (value: number | undefined) => void;
}) {
  return (
    <div className="panel">
      <div className="mb-1 text-xs font-semibold uppercase tracking-[0.14em] text-silver/70">
        {label}
      </div>
      <div className="mb-3 flex items-baseline justify-between gap-3">
        <div className="font-mono text-2xl font-bold text-oracle">
          {value !== undefined ? value : '—'}
        </div>
        {source ? (
          <span className="badge badge-oracle whitespace-nowrap">{source}</span>
        ) : null}
      </div>
      <input
        className="input-base font-mono"
        type="number"
        min={1}
        placeholder="Optional manual value"
        value={value ?? ''}
        onChange={(e) => onChange(Number(e.target.value) || undefined)}
      />
      {hint ? <p className="mt-2 text-[11px] text-silver/60">{hint}</p> : null}
    </div>
  );
}
