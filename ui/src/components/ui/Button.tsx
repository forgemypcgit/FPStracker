import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { clsx } from 'clsx';
import { Loader2 } from 'lucide-react';

type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  icon?: ReactNode;
  iconPosition?: 'left' | 'right';
  fullWidth?: boolean;
}

const variantStyles: Record<ButtonVariant, string> = {
  primary: [
    'bg-oracle text-void font-bold',
    'hover:bg-oracle/90',
  ].join(' '),
  secondary: [
    'border border-ash/50 bg-transparent text-pearl',
    'hover:border-ash hover:text-pearl',
  ].join(' '),
  ghost: [
    'bg-transparent text-silver/70',
    'hover:text-pearl hover:bg-smoke/40',
  ].join(' '),
  danger: [
    'border border-critical/30 bg-critical/10 text-critical',
    'hover:bg-critical/20 hover:border-critical/40',
  ].join(' '),
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: 'px-3 py-1.5 text-xs rounded',
  md: 'px-4 py-2 text-sm rounded',
  lg: 'px-6 py-3 text-base rounded',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      className,
      variant = 'primary',
      size = 'md',
      loading = false,
      disabled,
      icon,
      iconPosition = 'left',
      fullWidth = false,
      children,
      ...props
    },
    ref
  ) => {
    const isDisabled = disabled || loading;

    return (
      <button
        ref={ref}
        disabled={isDisabled}
        className={clsx(
          'relative inline-flex items-center justify-center font-semibold transition-all duration-200',
          'disabled:pointer-events-none disabled:opacity-40',
          variantStyles[variant],
          sizeStyles[size],
          fullWidth && 'w-full',
          className
        )}
        {...props}
      >
        {variant === 'primary' && (
          <span
            className="pointer-events-none absolute inset-0 rounded-[inherit] bg-gradient-to-b from-white/10 to-transparent opacity-100"
            aria-hidden
          />
        )}
        {loading ? (
          <Loader2 className={clsx('h-4 w-4 animate-spin', size === 'sm' && 'h-3.5 w-3.5')} />
        ) : (
          icon && iconPosition === 'left' && (
            <span className="flex-shrink-0">{icon}</span>
          )
        )}
        {!loading && children}
        {!loading && icon && iconPosition === 'right' && (
          <span className="flex-shrink-0">{icon}</span>
        )}
      </button>
    );
  }
);

Button.displayName = 'Button';

export const IconButton = forwardRef<HTMLButtonElement, Omit<ButtonProps, 'children' | 'icon'>>(
  ({ className, size = 'md', variant = 'ghost', ...props }, ref) => {
    const sizeMap = {
      sm: 'h-8 w-8',
      md: 'h-10 w-10',
      lg: 'h-12 w-12',
    };

    return (
      <Button
        ref={ref}
        variant={variant}
        size={size}
        className={clsx('!p-0', sizeMap[size], className)}
        {...props}
      />
    );
  }
);

IconButton.displayName = 'IconButton';

interface ButtonGroupProps {
  children: ReactNode;
  className?: string;
  orientation?: 'horizontal' | 'vertical';
}

export function ButtonGroup({ children, className, orientation = 'horizontal' }: ButtonGroupProps) {
  return (
    <div
      className={clsx(
        'flex',
        orientation === 'horizontal' ? 'flex-row gap-2' : 'flex-col gap-2',
        className
      )}
    >
      {children}
    </div>
  );
}
