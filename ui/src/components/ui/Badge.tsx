import { type HTMLAttributes, type ReactNode } from 'react';
import { clsx } from 'clsx';

type BadgeVariant = 'default' | 'oracle' | 'optimal' | 'caution' | 'critical' | 'soft';

interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  variant?: BadgeVariant;
  icon?: ReactNode;
  size?: 'sm' | 'md';
}

const badgeVariants: Record<BadgeVariant, string> = {
  default: 'bg-smoke/60 text-silver',
  oracle: 'bg-oracle/10 text-oracle',
  optimal: 'bg-optimal/10 text-optimal',
  caution: 'bg-caution/10 text-caution',
  critical: 'bg-critical/10 text-critical',
  soft: 'bg-smoke/30 text-silver/70',
};

const badgeSizes = {
  sm: 'px-2 py-0.5 text-[10px]',
  md: 'px-2.5 py-1 text-[11px]',
};

export function Badge({
  className,
  variant = 'default',
  icon,
  size = 'md',
  children,
  ...props
}: BadgeProps) {
  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1 rounded-full font-semibold tracking-wide',
        badgeVariants[variant],
        badgeSizes[size],
        className
      )}
      {...props}
    >
      {icon}
      {children}
    </span>
  );
}

interface StatusBadgeProps {
  status: 'ok' | 'review' | 'missing' | 'pending';
  label?: string;
  className?: string;
}

const statusConfig = {
  ok: { variant: 'optimal' as const, text: 'Looks good' },
  review: { variant: 'caution' as const, text: 'Review recommended' },
  missing: { variant: 'critical' as const, text: 'Required' },
  pending: { variant: 'default' as const, text: 'Pending' },
};

export function StatusBadge({ status, label, className }: StatusBadgeProps) {
  const config = statusConfig[status];
  return (
    <Badge variant={config.variant} className={className}>
      {label || config.text}
    </Badge>
  );
}

interface CounterBadgeProps {
  count: number;
  max?: number;
  variant?: BadgeVariant;
  className?: string;
}

export function CounterBadge({ count, max = 99, variant = 'oracle', className }: CounterBadgeProps) {
  const display = count > max ? `${max}+` : String(count);
  return (
    <Badge variant={variant} size="sm" className={className}>
      {display}
    </Badge>
  );
}

interface TagProps extends HTMLAttributes<HTMLSpanElement> {
  removable?: boolean;
  onRemove?: () => void;
}

export function Tag({ className, removable, onRemove, children, ...props }: TagProps) {
  return (
    <span
      className={clsx(
        'inline-flex items-center gap-1.5 rounded-lg bg-smoke/30 px-2.5 py-1 text-xs font-medium text-silver',
        className
      )}
      {...props}
    >
      {children}
      {removable && (
        <button
          type="button"
          onClick={onRemove}
          className="flex h-4 w-4 items-center justify-center rounded hover:bg-smoke/50"
        >
          <svg className="h-3 w-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      )}
    </span>
  );
}
