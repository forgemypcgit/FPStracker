import { type HTMLAttributes, type ReactNode } from 'react';
import { clsx } from 'clsx';
import { ArrowLeft, X } from 'lucide-react';
import { Link } from 'react-router-dom';

interface PageHeaderProps extends HTMLAttributes<HTMLDivElement> {
  title: string;
  description?: string;
  backTo?: string;
  backLabel?: string;
  actions?: ReactNode;
  badge?: ReactNode;
  icon?: ReactNode;
  iconColor?: string;
}

export function PageHeader({
  className,
  title,
  description,
  backTo,
  backLabel = 'Back',
  actions,
  badge,
  icon,
  iconColor = 'text-oracle',
  ...props
}: PageHeaderProps) {
  return (
    <div className={clsx('mb-6', className)} {...props}>
      {backTo && (
        <Link
          to={backTo}
          className="mb-4 inline-flex items-center gap-1.5 text-sm text-silver transition-colors hover:text-pearl"
        >
          <ArrowLeft className="h-4 w-4" />
          {backLabel}
        </Link>
      )}

      <div className="flex items-start gap-4">
        {icon && (
          <div className={clsx('flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-smoke/30', iconColor)}>
            {icon}
          </div>
        )}

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-semibold text-white">{title}</h1>
            {badge}
          </div>
          {description && <p className="mt-1 text-sm text-silver">{description}</p>}
        </div>

        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>
    </div>
  );
}

interface PageFooterProps extends HTMLAttributes<HTMLDivElement> {
  primaryAction?: ReactNode;
  secondaryActions?: ReactNode;
}

export function PageFooter({ className, primaryAction, secondaryActions, ...props }: PageFooterProps) {
  return (
    <div
      className={clsx('mt-8 flex flex-col-reverse gap-3 sm:flex-row sm:items-center sm:justify-between', className)}
      {...props}
    >
      <div className="flex flex-wrap items-center gap-2">{secondaryActions}</div>
      {primaryAction}
    </div>
  );
}

interface ProgressJourneyProps {
  steps: Array<{
    id: string;
    label: string;
    path?: string;
  }>;
  currentStep: string;
  className?: string;
}

export function ProgressJourney({ steps, currentStep, className }: ProgressJourneyProps) {
  const currentIndex = steps.findIndex((s) => s.id === currentStep);

  return (
    <nav className={clsx('flex items-center', className)} aria-label="Progress">
      <ol className="flex items-center">
        {steps.map((step, index) => {
          const isComplete = index < currentIndex;
          const isCurrent = index === currentIndex;
          const isLast = index === steps.length - 1;

          return (
            <li key={step.id} className="flex items-center">
              <div className="flex items-center">
                <div
                  className={clsx(
                    'relative flex h-8 w-8 items-center justify-center rounded-full text-xs font-bold transition-all duration-300',
                    isComplete && 'bg-optimal text-void',
                    isCurrent && 'bg-oracle text-void shadow-oracle-subtle',
                    !isComplete && !isCurrent && 'border border-ash/50 bg-smoke/30 text-silver/50'
                  )}
                >
                  {isComplete ? (
                    <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    index + 1
                  )}
                  {isCurrent && (
                    <span className="absolute inset-0 rounded-full border-2 border-oracle/30 animate-glow-pulse" />
                  )}
                </div>
                <span
                  className={clsx(
                    'ml-2 text-xs font-semibold uppercase tracking-wider transition-colors',
                    (isComplete || isCurrent) ? 'text-white' : 'text-silver/50',
                    'hidden sm:inline'
                  )}
                >
                  {step.label}
                </span>
              </div>

              {!isLast && (
                <div className="relative mx-2 h-px w-4 overflow-hidden sm:mx-3 sm:w-8">
                  <div
                    className={clsx(
                      'absolute inset-0 h-full transition-all duration-500',
                      isComplete ? 'bg-optimal' : 'bg-ash/30'
                    )}
                  />
                  {isComplete && (
                    <div className="absolute inset-0 h-full animate-shimmer bg-gradient-to-r from-optimal/0 via-white/30 to-optimal/0" />
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ol>

      <div className="ml-auto flex items-center gap-2">
        <Link
          to="/feedback"
          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-silver transition-colors hover:bg-smoke/40 hover:text-pearl"
        >
          <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
            />
          </svg>
          <span className="hidden sm:inline">Feedback</span>
        </Link>
        <Link
          to="/"
          className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-silver transition-colors hover:bg-smoke/40 hover:text-pearl"
        >
          <X className="h-3.5 w-3.5" />
          <span className="hidden sm:inline">Exit</span>
        </Link>
      </div>
    </nav>
  );
}

// Full journey steps (5 steps)
const fullJourneySteps = [
  { id: 'detect', label: 'Detect' },
  { id: 'synthetic', label: 'Baseline' },
  { id: 'game', label: 'Game' },
  { id: 'benchmark', label: 'Results' },
  { id: 'review', label: 'Review' },
];

// Contribute flow steps (3 steps - for game/benchmark/review pages)
const contributeSteps = [
  { id: 'game', label: 'Game' },
  { id: 'benchmark', label: 'Results' },
  { id: 'review', label: 'Review' },
];

interface ContributionLayoutProps {
  children: ReactNode;
  currentStep: 'game' | 'benchmark' | 'review';
}

export function ContributionLayout({ children, currentStep }: ContributionLayoutProps) {
  return (
    <div className="page-wrap animate-soft-slide">
      <ProgressJourney steps={contributeSteps} currentStep={currentStep} className="mb-10" />
      {children}
    </div>
  );
}

interface FullJourneyLayoutProps {
  children: ReactNode;
  currentStep: 'detect' | 'synthetic' | 'game' | 'benchmark' | 'review';
}

export function FullJourneyLayout({ children, currentStep }: FullJourneyLayoutProps) {
  return (
    <div className="page-wrap animate-soft-slide">
      <ProgressJourney steps={fullJourneySteps} currentStep={currentStep} className="mb-10" />
      {children}
    </div>
  );
}
