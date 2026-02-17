import { type ReactNode } from 'react';
import { clsx } from 'clsx';
import { Link } from 'react-router-dom';

interface ProgressStepperProps {
  steps: string[];
  currentStep: number;
}

export function ProgressStepper({ steps, currentStep }: ProgressStepperProps) {
  return (
    <div className="mb-8">
      <div className="mb-3 font-mono text-xs text-silver/60">FPS_TRACKER // CONTRIBUTE</div>
      <div className="flex items-center gap-2">
        {steps.map((label, index) => {
          const stepNum = index + 1;
          const isActive = stepNum === currentStep;
          const isComplete = stepNum < currentStep;

          return (
            <div key={label} className="flex items-center gap-2">
              <div
                className={clsx(
                  'step-dot',
                  isComplete && 'step-complete',
                  isActive && 'step-active',
                  !isComplete && !isActive && 'step-pending'
                )}
              >
                {isComplete ? (
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  stepNum
                )}
              </div>
                <span className={clsx(
                  'text-xs font-mono hidden sm:inline',
                  isActive ? 'text-oracle' : isComplete ? 'text-pearl' : 'text-silver/60'
                )}>
                  {label}
                </span>
              {index < steps.length - 1 && (
                <div className={clsx(
                  'h-px w-6 sm:w-8',
                  isComplete ? 'bg-optimal' : 'bg-ash'
                )} />
              )}
            </div>
          );
        })}
        <div className="ml-auto flex gap-2">
          <Link to="/feedback" className="text-xs text-silver/60 transition hover:text-pearl">
            Feedback
          </Link>
          <Link to="/" className="text-xs text-silver/60 transition hover:text-pearl">
            Exit
          </Link>
        </div>
      </div>
    </div>
  );
}

interface TerminalLayoutProps {
  children: ReactNode;
  currentStep: number;
}

const fullSteps = ['DETECT', 'BASELINE', 'GAME', 'RESULTS', 'REVIEW'];

export function TerminalLayout({ children, currentStep }: TerminalLayoutProps) {
  return (
    <div className="page-wrap animate-soft-slide">
      <ProgressStepper steps={fullSteps} currentStep={currentStep} />
      {children}
    </div>
  );
}

interface PageSectionProps {
  title: string;
  children: ReactNode;
}

export function PageSection({ title, children }: PageSectionProps) {
  return (
    <div className="mb-6">
      <h2 className="text-lg font-bold mb-4 font-mono">// {title}</h2>
      {children}
    </div>
  );
}
