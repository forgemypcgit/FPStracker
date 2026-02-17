import { forwardRef, type HTMLAttributes, type ReactNode } from 'react';
import { clsx } from 'clsx';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  variant?: 'default' | 'glow' | 'elevated' | 'soft';
  hover?: boolean;
  padding?: 'none' | 'sm' | 'md' | 'lg';
}

const cardVariants = {
  default: 'border-ash/20 bg-obsidian/60 shadow-[0_4px_24px_rgba(0,0,0,0.15),inset_0_1px_0_0_rgba(255,255,255,0.02)]',
  glow: 'border-ash/20 bg-obsidian/60 shadow-[0_4px_24px_rgba(0,0,0,0.15),0_0_0_1px_rgba(25,212,255,0.04),inset_0_1px_0_0_rgba(255,255,255,0.02)]',
  elevated: 'border-ash/15 bg-obsidian shadow-[0_8px_40px_rgba(0,0,0,0.3),0_2px_8px_rgba(0,0,0,0.15)]',
  soft: 'border-ash/15 bg-smoke/30 shadow-[inset_0_1px_0_0_rgba(255,255,255,0.02)]',
};

const cardPadding = {
  none: '',
  sm: 'p-3',
  md: 'p-4',
  lg: 'p-6',
};

export const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, variant = 'default', hover = false, padding = 'md', children, ...props }, ref) => {
    return (
      <div
        ref={ref}
        className={clsx(
          'relative rounded-2xl border backdrop-blur-sm',
          cardVariants[variant],
          cardPadding[padding],
          hover && 'transition-all duration-300 hover:-translate-y-0.5 hover:border-ash/30 hover:shadow-[0_8px_32px_rgba(0,0,0,0.2),inset_0_1px_0_0_rgba(255,255,255,0.03)]',
          className
        )}
        {...props}
      >
        {variant === 'glow' && (
          <span
            className="pointer-events-none absolute -inset-px rounded-2xl bg-gradient-to-br from-oracle/10 via-transparent to-optimal/5 opacity-100"
            style={{
              mask: 'linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)',
              WebkitMask: 'linear-gradient(#fff 0 0) content-box, linear-gradient(#fff 0 0)',
              maskComposite: 'exclude',
              WebkitMaskComposite: 'xor',
              padding: '1px',
            }}
            aria-hidden
          />
        )}
        {children}
      </div>
    );
  }
);

Card.displayName = 'Card';

interface CardHeaderProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: ReactNode;
  description?: ReactNode;
  action?: ReactNode;
  icon?: ReactNode;
  iconColor?: string;
}

export function CardHeader({
  className,
  title,
  description,
  action,
  icon,
  iconColor = 'text-oracle',
  children,
  ...props
}: CardHeaderProps) {
  return (
    <div className={clsx('flex items-start gap-3', className)} {...props}>
      {icon && (
        <div className={clsx('flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-smoke/40', iconColor)}>
          {icon}
        </div>
      )}
      <div className="min-w-0 flex-1">
        {title && <h3 className="text-lg font-semibold text-white">{title}</h3>}
        {description && <p className="mt-0.5 text-sm text-silver">{description}</p>}
        {children}
      </div>
      {action && <div className="shrink-0">{action}</div>}
    </div>
  );
}

interface CardSectionProps extends Omit<HTMLAttributes<HTMLDivElement>, 'title'> {
  title?: ReactNode;
  collapsible?: boolean;
  defaultOpen?: boolean;
}

export function CardSection({
  className,
  title,
  children,
  ...props
}: CardSectionProps) {
  return (
    <div className={clsx('border-t border-ash/15 pt-4 first:border-t-0 first:pt-0', className)} {...props}>
      {title && (
        <h4 className="mb-3 text-xs font-bold uppercase tracking-[0.12em] text-silver/60">{title}</h4>
      )}
      {children}
    </div>
  );
}

interface MetricCardProps {
  label: string;
  value: ReactNode;
  subtitle?: ReactNode;
  status?: 'ok' | 'warning' | 'error' | 'pending';
  trend?: 'up' | 'down' | 'neutral';
  className?: string;
}

const statusStyles = {
  ok: 'border-optimal/20 bg-optimal/5',
  warning: 'border-caution/20 bg-caution/5',
  error: 'border-critical/20 bg-critical/5',
  pending: 'border-ash/15 bg-smoke/20',
};

const statusValueColors = {
  ok: 'text-optimal',
  warning: 'text-caution',
  error: 'text-critical',
  pending: 'text-silver',
};

export function MetricCard({ label, value, subtitle, status = 'pending', className }: MetricCardProps) {
  return (
    <div
      className={clsx(
        'rounded-xl border p-3 transition-all duration-200',
        statusStyles[status],
        className
      )}
    >
      <div className="text-[10px] font-semibold uppercase tracking-[0.15em] text-silver/50">{label}</div>
      <div className={clsx('mt-1 font-mono text-xl font-bold', statusValueColors[status])}>{value}</div>
      {subtitle && <div className="mt-0.5 text-[11px] text-silver/60">{subtitle}</div>}
    </div>
  );
}

interface SpecCardProps {
  title: string;
  icon: ReactNode;
  iconColor?: string;
  borderColor?: string;
  status?: 'ok' | 'review' | 'missing';
  children: ReactNode;
  className?: string;
}

const specStatusStyles = {
  ok: 'border-optimal/30',
  review: 'border-caution/30',
  missing: 'border-critical/30',
};

export function SpecCard({
  title,
  icon,
  iconColor = 'text-oracle',
  borderColor,
  status,
  children,
  className,
}: SpecCardProps) {
  return (
    <Card
      variant="default"
      padding="none"
      className={clsx(
        'overflow-hidden border-t-2',
        borderColor || (status && specStatusStyles[status]),
        className
      )}
    >
      <div className="flex items-center gap-2.5 border-b border-ash/15 px-4 py-3">
        <div className={clsx('flex h-8 w-8 items-center justify-center rounded-lg bg-smoke/40', iconColor)}>
          {icon}
        </div>
        <h3 className="text-base font-semibold text-white">{title}</h3>
      </div>
      <div className="p-4">{children}</div>
    </Card>
  );
}

interface GameCardProps {
  name: string;
  subtitle?: ReactNode;
  badges?: ReactNode;
  selected?: boolean;
  onClick?: () => void;
  className?: string;
}

export function GameCard({ name, subtitle, badges, selected, onClick, className }: GameCardProps) {
  return (
    <Card
      variant={selected ? 'glow' : 'default'}
      hover={!selected}
      padding="md"
      className={clsx(
        'cursor-pointer',
        selected && 'border-oracle/30',
        className
      )}
      onClick={onClick}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <h4 className="text-sm font-medium text-white">{name}</h4>
          {subtitle && <p className="mt-0.5 text-xs text-silver/70">{subtitle}</p>}
          {badges && <div className="mt-2 flex flex-wrap gap-1.5">{badges}</div>}
        </div>
        <div className={clsx(
          'flex h-6 w-6 shrink-0 items-center justify-center rounded-full',
          selected ? 'bg-oracle text-void' : 'bg-smoke/40 text-silver/40'
        )}>
          <svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
          </svg>
        </div>
      </div>
    </Card>
  );
}
