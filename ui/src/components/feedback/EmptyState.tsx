import { type HTMLAttributes, type ReactNode } from 'react';
import { clsx } from 'clsx';

interface EmptyStateProps extends HTMLAttributes<HTMLDivElement> {
  icon?: ReactNode;
  title: string;
  description?: string;
  action?: ReactNode;
  size?: 'sm' | 'md' | 'lg';
}

const emptyStateSizes = {
  sm: {
    container: 'py-6',
    icon: 'h-12 w-12',
    title: 'text-base',
    description: 'text-xs',
  },
  md: {
    container: 'py-10',
    icon: 'h-16 w-16',
    title: 'text-lg',
    description: 'text-sm',
  },
  lg: {
    container: 'py-16',
    icon: 'h-20 w-20',
    title: 'text-xl',
    description: 'text-sm',
  },
};

export function EmptyState({
  className,
  icon,
  title,
  description,
  action,
  size = 'md',
  children,
  ...props
}: EmptyStateProps) {
  const sizes = emptyStateSizes[size];

  return (
    <div className={clsx('flex flex-col items-center justify-center text-center', sizes.container, className)} {...props}>
      {icon && (
        <div className={clsx('mb-4 flex items-center justify-center rounded-2xl bg-smoke/30', sizes.icon)}>
          <div className="text-silver/50">{icon}</div>
        </div>
      )}
      <h3 className={clsx('font-semibold text-pearl', sizes.title)}>{title}</h3>
      {description && (
        <p className={clsx('mt-2 max-w-sm text-silver/60', sizes.description)}>{description}</p>
      )}
      {action && <div className="mt-4">{action}</div>}
      {children}
    </div>
  );
}

interface LoadingStateProps extends HTMLAttributes<HTMLDivElement> {
  message?: string;
}

export function LoadingState({ className, message = 'Loading...', ...props }: LoadingStateProps) {
  return (
    <div className={clsx('flex flex-col items-center justify-center py-12', className)} {...props}>
      <div className="h-10 w-10 animate-spin rounded-full border-2 border-ash border-t-oracle" />
      <p className="mt-4 text-sm text-silver">{message}</p>
    </div>
  );
}

interface ErrorStateProps extends HTMLAttributes<HTMLDivElement> {
  title?: string;
  message: string;
  retry?: () => void;
  retryLabel?: string;
}

export function ErrorState({
  className,
  title = 'Something went wrong',
  message,
  retry,
  retryLabel = 'Try again',
  ...props
}: ErrorStateProps) {
  return (
    <EmptyState
      className={className}
      icon={
        <svg className="h-8 w-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
          />
        </svg>
      }
      title={title}
      description={message}
      action={
        retry && (
          <button
            type="button"
            onClick={retry}
            className="btn-secondary mt-4"
          >
            {retryLabel}
          </button>
        )
      }
      {...props}
    />
  );
}
