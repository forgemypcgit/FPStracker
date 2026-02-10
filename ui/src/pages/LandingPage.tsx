import { Link } from "react-router-dom";
import { ArrowRight, Database, Gauge, ShieldCheck, Zap, ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

export default function LandingPage() {
  return (
    <div className="page-wrap">
      {/* Ambient orbs */}
      <div className="orb orb-oracle -top-32 left-1/4 h-96 w-96 animate-float opacity-60" />
      <div className="orb orb-optimal -right-20 top-40 h-72 w-72 animate-float-delayed opacity-40" />
      <div className="orb orb-caution -left-16 bottom-20 h-56 w-56 animate-float-slow opacity-30" />

      {/* Hero */}
      <section className="relative pb-16 pt-6 sm:pt-14">
        <div className="mb-6 inline-flex items-center gap-2 rounded-full border border-oracle/20 bg-oracle/[0.06] px-4 py-1.5">
          <Zap className="h-3.5 w-3.5 text-oracle" />
          <span className="text-xs font-semibold tracking-wide text-oracle">
            PC Builder FPS Tracker
          </span>
        </div>

        <h1 className="max-w-3xl text-4xl font-bold leading-[1.08] sm:text-5xl lg:text-6xl">
          <span className="text-white">Contribute a</span>
          <br />
          <span className="gradient-text">benchmark</span>
          <span className="text-white"> in&nbsp;minutes.</span>
        </h1>

        <p className="mt-6 max-w-xl text-base leading-relaxed text-silver sm:text-lg">
          Hardware + FPS data that improves build recommendations for everyone.
          No invasive hooks. No personal data. Just real performance numbers.
        </p>

        <div className="mt-10 flex flex-wrap items-center gap-4">
          <Link to="/terms" className="btn-primary group">
            Start Contribution
            <ArrowRight className="h-4 w-4 transition-transform duration-200 group-hover:translate-x-0.5" />
          </Link>
          <Link to="/import" className="btn-secondary group">
            Import Existing Log
            <ChevronRight className="h-4 w-4 text-silver transition-transform duration-200 group-hover:translate-x-0.5" />
          </Link>
        </div>

      </section>

      {/* Divider */}
      <div className="divider mb-10" />

      {/* Info cards */}
      <section className="stagger-children grid gap-5 sm:grid-cols-3">
        <InfoCard
          icon={<ShieldCheck className="h-5 w-5" />}
          iconColor="text-oracle"
          glowColor="bg-oracle/10"
          title="Safe Method"
          text="Manual and tool-assisted capture with anti-cheat risk labeling per game."
        />
        <InfoCard
          icon={<Gauge className="h-5 w-5" />}
          iconColor="text-optimal"
          glowColor="bg-optimal/10"
          title="Fast Flow"
          text="Most submissions take under 3 minutes. Auto-detect hardware, pick a game, enter FPS."
        />
        <InfoCard
          icon={<Database className="h-5 w-5" />}
          iconColor="text-caution"
          glowColor="bg-caution/10"
          title="Real Impact"
          text="Your benchmark improves prediction quality for every future PC build recommendation."
        />
      </section>
    </div>
  );
}

interface InfoCardProps {
  icon: ReactNode;
  iconColor: string;
  glowColor: string;
  title: string;
  text: string;
}

function InfoCard({ icon, iconColor, glowColor, title, text }: InfoCardProps) {
  return (
    <article className="spotlight-card panel-glow group transition-all duration-300 hover:-translate-y-0.5">
      <div className={`mb-4 flex h-10 w-10 items-center justify-center rounded-xl ${glowColor} ${iconColor}`}>
        {icon}
      </div>
      <h3 className="text-sm font-semibold text-white">{title}</h3>
      <p className="mt-2 text-sm leading-relaxed text-silver">{text}</p>
    </article>
  );
}
