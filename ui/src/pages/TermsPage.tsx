import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Check, Shield, FileText, AlertTriangle, ArrowRight } from "lucide-react";
import type { ReactNode } from "react";

export default function TermsPage() {
  const navigate = useNavigate();
  const [acceptTos, setAcceptTos] = useState(false);
  const [acceptTraining, setAcceptTraining] = useState(false);
  const [acceptRetention, setAcceptRetention] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const allAccepted = acceptTos && acceptTraining && acceptRetention;
  const acceptedCount = [acceptTos, acceptTraining, acceptRetention].filter(Boolean).length;

  async function handleContinue() {
    if (!allAccepted || submitting) return;
    setSubmitError(null);
    setSubmitting(true);

    try {
      const response = await fetch("/api/consent/accept", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          tos_accepted: acceptTos,
          consent_public_use: acceptTraining,
          retention_acknowledged: acceptRetention,
        }),
      });

      if (!response.ok) {
        const text = await response.text().catch(() => "");
        console.warn("Consent endpoint returned non-OK response.", {
          status: response.status,
          body: text,
        });
        throw new Error("Failed to record consent. Please try again.");
      }

      navigate("/detect");
    } catch (err) {
      console.error("Failed to record consent.", err);
      setSubmitError("Failed to record consent. Please try again.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="page-wrap animate-soft-slide">
      <section className="panel-glow">
        <h1 className="text-3xl font-semibold text-white">Consent & Data Terms</h1>
        <p className="mt-2 text-sm text-silver">
          Review and confirm before continuing.
        </p>

        {/* Progress indicator */}
        <div className="mt-5 flex items-center gap-3">
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-smoke/60">
            <div
              className="h-full rounded-full bg-oracle transition-all duration-500 ease-out"
              style={{ width: `${(acceptedCount / 3) * 100}%` }}
            />
          </div>
          <span className="text-xs font-mono text-silver/60">{acceptedCount}/3</span>
        </div>

        <div className="mt-6 stagger-children space-y-3">
          <SummaryCard
            icon={<Shield className="h-4 w-4 text-oracle" />}
            title="What we collect"
            text="Hardware specs, benchmark settings, and FPS metrics."
          />
          <SummaryCard
            icon={<FileText className="h-4 w-4 text-optimal" />}
            title="What we skip"
            text="No serial numbers, file lists, or personal profile data."
          />
          <SummaryCard
            icon={<AlertTriangle className="h-4 w-4 text-caution" />}
            title="Retention"
            text="Records may be retained up to 10 years. You can exit before submission."
          />
        </div>

        <div className="divider my-6" />

        <div className="space-y-2.5">
          <AgreementRow
            checked={acceptTos}
            onToggle={() => setAcceptTos((v) => !v)}
            label="I agree to the Terms of Service."
          />
          <AgreementRow
            checked={acceptTraining}
            onToggle={() => setAcceptTraining((v) => !v)}
            label="I consent to benchmark data being used publicly (including aggregate stats and commercial use)."
          />
          <AgreementRow
            checked={acceptRetention}
            onToggle={() => setAcceptRetention((v) => !v)}
            label="I understand retention may be up to 10 years."
          />
        </div>

        {submitError && (
          <div className="mt-4 rounded-xl border border-critical/40 bg-critical/[0.06] px-4 py-3 text-sm text-pearl">
            {submitError}
          </div>
        )}

        <div className="mt-8 flex flex-wrap justify-end gap-3">
          <Link to="/" className="btn-secondary">
            Cancel
          </Link>
          <button
            type="button"
            disabled={!allAccepted || submitting}
            onClick={handleContinue}
            className="btn-primary group"
          >
            {submitting ? "Saving..." : "Continue"}
            <ArrowRight className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />
          </button>
        </div>
      </section>
    </div>
  );
}

interface SummaryCardProps {
  icon: ReactNode;
  title: string;
  text: string;
}

function SummaryCard({ icon, title, text }: SummaryCardProps) {
  return (
    <div className="flex items-start gap-3 rounded-xl bg-smoke/30 px-4 py-3.5 transition-colors duration-200 hover:bg-smoke/40">
      <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-smoke/60">
        {icon}
      </div>
      <div>
        <p className="text-sm font-medium text-white">{title}</p>
        <p className="mt-0.5 text-sm text-silver">{text}</p>
      </div>
    </div>
  );
}

interface AgreementRowProps {
  checked: boolean;
  onToggle: () => void;
  label: string;
}

function AgreementRow({ checked, onToggle, label }: AgreementRowProps) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={`flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all duration-200 ${
        checked
          ? "border-oracle/40 bg-oracle/[0.06]"
          : "border-ash/40 bg-smoke/30 hover:border-ash/60"
      }`}
    >
      <span
        className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-md border transition-all duration-200 ${
          checked ? "border-oracle bg-oracle text-void" : "border-ash bg-abyss"
        }`}
      >
        {checked && <Check className="h-3 w-3" />}
      </span>
      <span className="text-sm text-pearl">{label}</span>
    </button>
  );
}
