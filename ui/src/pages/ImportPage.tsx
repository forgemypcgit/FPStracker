import { useState } from "react";
import { Link } from "react-router-dom";
import { CheckCircle2, FileUp, Loader2, Upload, ArrowRight } from "lucide-react";

type ParseStatus = "idle" | "parsing" | "done" | "error";

export default function ImportPage() {
  const [fileName, setFileName] = useState<string | null>(null);
  const [status, setStatus] = useState<ParseStatus>("idle");
  const [isDragging, setIsDragging] = useState(false);

  const onPick = (file: File) => {
    setFileName(file.name);
    setStatus("parsing");
    setTimeout(() => setStatus("done"), 1200);
  };

  return (
    <div className="page-wrap animate-soft-slide">
      <section className="panel-glow">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-oracle/10">
            <Upload className="h-5 w-5 text-oracle" />
          </div>
          <div>
            <h1 className="text-2xl font-semibold text-white sm:text-3xl">
              Import Benchmark Log
            </h1>
            <p className="mt-1.5 text-sm text-silver">
              Supports CapFrameX CSV and MangoHud logs.
            </p>
          </div>
        </div>

        <label
          className={`mt-6 flex cursor-pointer flex-col items-center gap-3 rounded-xl border-2 border-dashed p-12 text-center transition-all duration-300 ${
            isDragging
              ? "border-oracle/60 bg-oracle/[0.06]"
              : status === "done"
              ? "border-optimal/30 bg-optimal/[0.04]"
              : "border-ash/40 bg-smoke/20 hover:border-oracle/30 hover:bg-smoke/30"
          }`}
          onDragOver={(e) => {
            e.preventDefault();
            setIsDragging(true);
          }}
          onDragLeave={() => setIsDragging(false)}
          onDrop={(e) => {
            e.preventDefault();
            setIsDragging(false);
            const file = e.dataTransfer.files[0];
            if (file) onPick(file);
          }}
        >
          <input
            type="file"
            className="hidden"
            accept=".csv,.log"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) onPick(file);
            }}
          />

          {status === "idle" && (
            <>
              <div className="flex h-12 w-12 items-center justify-center rounded-full bg-smoke/60">
                <FileUp className="h-6 w-6 text-silver" />
              </div>
              <p className="text-sm text-silver">Drop a file or click to browse</p>
              <p className="text-[11px] text-silver/40">.csv, .log</p>
            </>
          )}

          {status === "parsing" && (
            <>
              <div className="flex h-12 w-12 items-center justify-center rounded-full bg-oracle/10">
                <Loader2 className="h-6 w-6 animate-spin text-oracle" />
              </div>
              <p className="text-sm text-silver">Parsing <span className="font-medium text-pearl">{fileName}</span>...</p>
            </>
          )}

          {status === "done" && (
            <>
              <div className="flex h-12 w-12 items-center justify-center rounded-full bg-optimal/10 animate-scale-in">
                <CheckCircle2 className="h-6 w-6 text-optimal" />
              </div>
              <p className="text-sm font-medium text-white">{fileName}</p>
              <p className="text-xs text-optimal">Ready to review</p>
            </>
          )}
        </label>
      </section>

      <div className="mt-6 flex flex-wrap justify-between gap-3">
        <Link to="/" className="btn-secondary">
          Cancel
        </Link>
        {status === "done" && (
          <Link to="/contribute/review" className="btn-primary group animate-fade-in-up">
            Continue
            <ArrowRight className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />
          </Link>
        )}
      </div>
    </div>
  );
}
