import { useEffect, useMemo, useState } from "react";
import { Link, useLocation } from "react-router-dom";
import { Bug, MessageSquareText, Shield, Send, ChevronLeft, Check } from "lucide-react";
import { motion } from "framer-motion";

import { clearIdempotencyKey, getOrCreateIdempotencyKey } from "@/lib/idempotency";

type FeedbackSurface = "web_ui" | "terminal_ui";
type OsFamily = "windows" | "macos" | "linux" | "other";

interface FeedbackIssueOption {
  code: string;
  label: string;
  hint: string;
}

interface FeedbackCategorySchema {
  id: string;
  label: string;
  description: string;
  issues: FeedbackIssueOption[];
}

interface FeedbackSchema {
  schema_version: number;
  surface: FeedbackSurface;
  os: OsFamily;
  intro: string;
  privacy_note: string;
  categories: FeedbackCategorySchema[];
}

interface FeedbackSubmitResponse {
  status: string;
  message: string;
}

function classNames(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

export default function FeedbackPage() {
  const location = useLocation();
  const [schema, setSchema] = useState<FeedbackSchema | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(null);
  const [selectedIssueCode, setSelectedIssueCode] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [includeDiagnostics, setIncludeDiagnostics] = useState(false);

  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<FeedbackSubmitResponse | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setLoadError(null);
      try {
        const res = await fetch("/api/feedback/schema?surface=web_ui");
        const text = await res.text();
        if (!res.ok) {
          throw new Error(text || `Failed to load feedback schema (HTTP ${res.status})`);
        }
        const parsed = JSON.parse(text) as FeedbackSchema;
        if (!cancelled) {
          setSchema(parsed);
          // Default to first category to reduce friction.
          const params = new URLSearchParams(location.search);
          const preCategory = params.get("category");
          const preIssue = params.get("issue");
          const preMessage = params.get("message");

          const initialCategory =
            preCategory && parsed.categories.some((c) => c.id === preCategory)
              ? preCategory
              : (parsed.categories[0]?.id ?? null);

          setSelectedCategoryId(initialCategory);
          if (preIssue) setSelectedIssueCode(preIssue);
          if (preMessage) setMessage(preMessage);
        }
      } catch (err) {
        if (!cancelled) {
          setLoadError(err instanceof Error ? err.message : "Unknown error");
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [location.search]);

  const selectedCategory = useMemo(() => {
    if (!schema || !selectedCategoryId) return null;
    return schema.categories.find((c) => c.id === selectedCategoryId) ?? null;
  }, [schema, selectedCategoryId]);

  useEffect(() => {
    // Reset issue selection when category changes to avoid mismatched codes.
    setSelectedIssueCode(null);
  }, [selectedCategoryId]);

  const draftSignature = useMemo(() => {
    return [
      `surface=web_ui`,
      `cat=${selectedCategoryId ?? ""}`,
      `issue=${selectedIssueCode ?? ""}`,
      `msg=${message.trim()}`,
      `diag=${includeDiagnostics ? 1 : 0}`,
    ].join("|");
  }, [includeDiagnostics, message, selectedCategoryId, selectedIssueCode]);

  const idempotencyKey = useMemo(
    () => getOrCreateIdempotencyKey(draftSignature),
    [draftSignature]
  );

  async function submit() {
    if (!schema || !selectedCategoryId) return;
    if (!selectedIssueCode) {
      setSubmitError("Pick the option that best matches your issue.");
      return;
    }
    const msg = message.trim();
    if (!msg) {
      setSubmitError("Please describe what happened.");
      return;
    }
    if (msg.length > 2000) {
      setSubmitError("Message is too long (max 2000 characters).");
      return;
    }

    setSubmitting(true);
    setSubmitError(null);
    try {
      const res = await fetch("/api/feedback/submit", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Idempotency-Key": idempotencyKey,
          "X-Idempotency-Key": idempotencyKey,
        },
        body: JSON.stringify({
          category: selectedCategoryId,
          issue_code: selectedIssueCode,
          message: msg,
          include_diagnostics: includeDiagnostics,
        }),
      });

      const raw = await res.text();
      if (!res.ok) {
        throw new Error(raw || `Failed to submit feedback (HTTP ${res.status})`);
      }

      let parsed: FeedbackSubmitResponse;
      try {
        parsed = JSON.parse(raw) as FeedbackSubmitResponse;
      } catch {
        throw new Error("Feedback sent, but the app returned an invalid response payload.");
      }

      clearIdempotencyKey(draftSignature);
      setReceipt(parsed);
    } catch (err) {
      setSubmitError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setSubmitting(false);
    }
  }

  if (loading) {
    return (
      <div className="page-wrap animate-soft-slide">
        <section className="panel-glow shimmer">
          <div className="h-6 w-48 rounded bg-smoke/60" />
          <div className="mt-3 h-4 w-96 rounded bg-smoke/40" />
          <div className="mt-10 grid gap-4 sm:grid-cols-2">
            <div className="h-56 rounded-2xl bg-smoke/30" />
            <div className="h-56 rounded-2xl bg-smoke/30" />
          </div>
        </section>
      </div>
    );
  }

  if (loadError || !schema) {
    return (
      <div className="page-wrap animate-soft-slide">
        <section className="panel-glow">
          <div className="flex items-center gap-2">
            <Bug className="h-4 w-4 text-caution" />
            <h1 className="text-2xl font-semibold text-white">Feedback</h1>
          </div>
          <p className="mt-2 text-sm text-silver">
            Couldn’t load the feedback form.
          </p>
          <div className="divider my-6" />
          <p className="text-sm text-caution">{loadError ?? "Unknown error"}</p>
          <div className="mt-6 flex flex-wrap gap-3">
            <Link to="/" className="btn-secondary">
              <ChevronLeft className="h-4 w-4" />
              Back
            </Link>
          </div>
        </section>
      </div>
    );
  }

  if (receipt) {
    const queued = receipt.status === "queued";
    return (
      <div className="page-wrap animate-soft-slide">
        <section className="panel-glow">
          <div className="flex items-center gap-2">
            <div className={classNames("flex h-10 w-10 items-center justify-center rounded-xl", queued ? "bg-caution/10" : "bg-optimal/10")}>
              <Check className={classNames("h-5 w-5", queued ? "text-caution" : "text-optimal")} strokeWidth={3} />
            </div>
            <div>
              <h1 className="text-2xl font-semibold text-white">
                {queued ? "Saved for retry" : "Feedback sent"}
              </h1>
              <p className="mt-1 text-sm text-silver">{receipt.message}</p>
            </div>
          </div>

          <div className="divider my-6" />

          <div className="panel-soft">
            <div className="section-label">Next Step</div>
            <p className="mt-2 text-sm text-silver">
              If this blocks you, open an issue with short reproduction steps.
            </p>
            <p className="mt-2 text-xs text-silver/60">
              We don’t require accounts in the app, so GitHub issues are the best way to follow up.
            </p>
          </div>

          <div className="mt-7 flex flex-wrap gap-3">
            <Link to="/" className="btn-secondary">
              <ChevronLeft className="h-4 w-4" />
              Back to home
            </Link>
            <a
              className="btn-ghost"
              href="https://github.com/forgemypcgit/FPStracker/issues"
              target="_blank"
              rel="noreferrer"
            >
              Open GitHub issues
            </a>
          </div>
        </section>
      </div>
    );
  }

  const messageCount = message.length;
  const messageOver = messageCount > 2000;

  return (
    <div className="page-wrap animate-soft-slide">
      <div className="orb orb-oracle -top-28 left-1/4 h-72 w-72 animate-float opacity-50" />
      <div className="orb orb-caution -right-20 top-24 h-64 w-64 animate-float-delayed opacity-30" />

      <section className="panel-glow">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-2 rounded-full border border-oracle/20 bg-oracle/[0.06] px-4 py-1.5">
              <MessageSquareText className="h-3.5 w-3.5 text-oracle" />
              <span className="text-xs font-semibold tracking-wide text-oracle">
                Feedback
              </span>
            </div>
            <h1 className="mt-4 text-3xl font-semibold text-white">Report a problem</h1>
            <p className="mt-2 max-w-2xl text-sm text-silver">{schema.intro}</p>
            <p className="mt-2 max-w-2xl text-xs text-silver/60">
              {schema.privacy_note}{" "}
              <span className="text-silver/40">(OS: {schema.os})</span>
            </p>
          </div>

          <Link
            to="/"
            className="btn-secondary"
          >
            <ChevronLeft className="h-4 w-4" />
            Back
          </Link>
        </div>

        <div className="divider my-7" />

        <div className="grid gap-6 lg:grid-cols-12">
          <div className="lg:col-span-5">
            <div className="section-label">Category</div>
            <div className="mt-3 space-y-2">
              {schema.categories.map((cat, idx) => {
                const active = cat.id === selectedCategoryId;
                return (
                  <button
                    key={cat.id}
                    type="button"
                    onClick={() => setSelectedCategoryId(cat.id)}
                    className={classNames(
                      "group w-full rounded-2xl border px-4 py-3 text-left transition-all duration-200",
                      active
                        ? "border-oracle/40 bg-oracle/[0.06] shadow-oracle-subtle"
                        : "border-ash/40 bg-smoke/30 hover:border-ash/70 hover:bg-smoke/50"
                    )}
                    aria-pressed={active}
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div>
                        <div className={classNames("text-sm font-semibold", active ? "text-white" : "text-pearl")}>
                          {cat.label}
                        </div>
                        <div className={classNames("mt-1 text-xs leading-relaxed", active ? "text-silver" : "text-silver/70")}>
                          {cat.description}
                        </div>
                      </div>
                      <div className={classNames("badge", active ? "badge-oracle" : "bg-smoke/70")}>
                        {String(idx + 1).padStart(2, "0")}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="lg:col-span-7">
            <div className="section-label">What happened</div>
            <div className="mt-3 grid gap-2 sm:grid-cols-2">
              {(selectedCategory?.issues ?? []).map((issue) => {
                const active = issue.code === selectedIssueCode;
                return (
                  <button
                    key={issue.code}
                    type="button"
                    onClick={() => setSelectedIssueCode(issue.code)}
                    className={classNames(
                      "rounded-2xl border px-4 py-3 text-left transition-all duration-200",
                      active
                        ? "border-optimal/40 bg-optimal/[0.06]"
                        : "border-ash/40 bg-smoke/30 hover:border-ash/70 hover:bg-smoke/50"
                    )}
                    aria-pressed={active}
                  >
                    <div className="flex items-start gap-3">
                      <div className={classNames("mt-0.5 flex h-6 w-6 items-center justify-center rounded-full border", active ? "border-optimal/40 bg-optimal/10" : "border-ash/40 bg-smoke/60")}>
                        <span className={classNames("h-2.5 w-2.5 rounded-full", active ? "bg-optimal" : "bg-ash/70")} />
                      </div>
                      <div>
                        <div className={classNames("text-sm font-semibold", active ? "text-white" : "text-pearl")}>
                          {issue.label}
                        </div>
                        <div className="mt-1 text-xs leading-relaxed text-silver/70">
                          {issue.hint}
                        </div>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>

            <div className="mt-5">
              <label className="label" htmlFor="feedback-message">
                Details
              </label>
              <textarea
                id="feedback-message"
                className={classNames("input-base min-h-[140px] resize-y", messageOver && "border-critical/60 focus:border-critical/60 focus:ring-critical/15")}
                placeholder="What did you try? What did you expect to happen? What happened instead?"
                value={message}
                onChange={(e) => setMessage(e.target.value)}
                maxLength={4000}
              />
              <div className="mt-2 flex items-center justify-between gap-3">
                <div className="flex items-center gap-2 text-xs text-silver/70">
                  <Shield className="h-3.5 w-3.5 text-silver/50" />
                  <span>
                    We don’t need personal info. Avoid emails, passwords, or private links.
                  </span>
                </div>
                <div className={classNames("text-xs font-mono", messageOver ? "text-critical" : "text-silver/60")}>
                  {messageCount}/2000
                </div>
              </div>
            </div>

            <div className="mt-5 rounded-2xl border border-ash/40 bg-smoke/30 px-4 py-3">
              <label className="flex cursor-pointer items-start gap-3">
                <input
                  type="checkbox"
                  checked={includeDiagnostics}
                  onChange={(e) => setIncludeDiagnostics(e.target.checked)}
                  className="mt-1 h-4 w-4 accent-[#19d4ff]"
                />
                <div>
                  <div className="text-sm font-semibold text-white">
                    Include diagnostics summary (optional)
                  </div>
                  <div className="mt-1 text-xs leading-relaxed text-silver/70">
                    Includes app version, OS, and tool availability. Excludes local file paths.
                  </div>
                </div>
              </label>
            </div>

            {submitError && (
              <motion.div
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                className="mt-4 rounded-2xl border border-critical/30 bg-critical/10 px-4 py-3 text-sm text-critical"
              >
                {submitError}
              </motion.div>
            )}

            <div className="mt-6 flex flex-wrap items-center gap-3">
              <button
                type="button"
                onClick={submit}
                className="btn-primary"
                disabled={submitting}
              >
                <Send className="h-4 w-4" />
                {submitting ? "Sending..." : "Send feedback"}
              </button>

              <a
                className="btn-ghost"
                href="https://github.com/forgemypcgit/FPStracker/issues"
                target="_blank"
                rel="noreferrer"
              >
                Prefer GitHub issues?
              </a>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
