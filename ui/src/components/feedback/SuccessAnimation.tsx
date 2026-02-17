import { useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { clsx } from 'clsx';
import { Check } from 'lucide-react';

interface SuccessAnimationProps {
  show: boolean;
  onComplete?: () => void;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const sizes = {
  sm: { container: 'h-12 w-12', icon: 'h-6 w-6', ring: 40 },
  md: { container: 'h-16 w-16', icon: 'h-8 w-8', ring: 52 },
  lg: { container: 'h-20 w-20', icon: 'h-10 w-10', ring: 64 },
};

export function SuccessAnimation({ show, onComplete, size = 'md', className }: SuccessAnimationProps) {
  const s = sizes[size];

  useEffect(() => {
    if (show && onComplete) {
      const timer = setTimeout(onComplete, 1500);
      return () => clearTimeout(timer);
    }
  }, [show, onComplete]);

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ scale: 0.5, opacity: 0 }}
          animate={{ scale: 1, opacity: 1 }}
          exit={{ scale: 0.8, opacity: 0 }}
          transition={{ type: 'spring', damping: 15, stiffness: 300 }}
          className={clsx('relative flex items-center justify-center rounded-full bg-optimal/10', s.container, className)}
        >
          <motion.svg
            className="absolute inset-0"
            viewBox={`0 0 ${s.ring * 2} ${s.ring * 2}`}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.1 }}
          >
            <motion.circle
              cx={s.ring}
              cy={s.ring}
              r={s.ring - 4}
              fill="none"
              stroke="rgba(121, 242, 166, 0.2)"
              strokeWidth="2"
              initial={{ pathLength: 0 }}
              animate={{ pathLength: 1 }}
              transition={{ duration: 0.6, delay: 0.1 }}
            />
          </motion.svg>

          <motion.div
            initial={{ scale: 0, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ type: 'spring', damping: 12, stiffness: 400, delay: 0.2 }}
          >
            <Check className={clsx('text-optimal', s.icon)} strokeWidth={3} />
          </motion.div>

          <motion.div
            className="absolute inset-0 rounded-full border-2 border-optimal/30"
            initial={{ scale: 1, opacity: 0.6 }}
            animate={{ scale: 1.5, opacity: 0 }}
            transition={{ duration: 0.6, delay: 0.3 }}
          />
        </motion.div>
      )}
    </AnimatePresence>
  );
}

interface SuccessOverlayProps {
  show: boolean;
  title?: string;
  message?: string;
  onComplete?: () => void;
  autoHideDuration?: number;
}

export function SuccessOverlay({
  show,
  title = 'Success!',
  message,
  onComplete,
  autoHideDuration = 2000,
}: SuccessOverlayProps) {
  useEffect(() => {
    if (show && onComplete) {
      const timer = setTimeout(onComplete, autoHideDuration);
      return () => clearTimeout(timer);
    }
  }, [show, onComplete, autoHideDuration]);

  return (
    <AnimatePresence>
      {show && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-void/80 backdrop-blur-sm"
        >
          <motion.div
            initial={{ scale: 0.9, opacity: 0, y: 20 }}
            animate={{ scale: 1, opacity: 1, y: 0 }}
            exit={{ scale: 0.9, opacity: 0, y: 20 }}
            transition={{ type: 'spring', damping: 20, stiffness: 300 }}
            className="flex flex-col items-center rounded-3xl bg-obsidian p-8 shadow-2xl"
          >
            <SuccessAnimation show={show} size="lg" />
            <motion.h2
              initial={{ opacity: 0, y: 10 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: 0.3 }}
              className="mt-4 text-xl font-semibold text-white"
            >
              {title}
            </motion.h2>
            {message && (
              <motion.p
                initial={{ opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ delay: 0.4 }}
                className="mt-2 text-sm text-silver"
              >
                {message}
              </motion.p>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

interface ConfettiPieceProps {
  delay: number;
  color: string;
  x: number;
}

function ConfettiPiece({ delay, color, x }: ConfettiPieceProps) {
  return (
    <motion.div
      className="absolute h-2 w-1.5 rounded-sm"
      style={{ backgroundColor: color, left: `${x}%` }}
      initial={{ y: 0, opacity: 1, rotate: 0 }}
      animate={{
        y: -200,
        opacity: 0,
        rotate: Math.random() * 720 - 360,
      }}
      transition={{
        duration: 1 + Math.random() * 0.5,
        delay,
        ease: 'easeOut',
      }}
    />
  );
}

interface ConfettiProps {
  show: boolean;
}

const confettiColors = ['#19d4ff', '#79f2a6', '#ffb454', '#ff6b6b', '#a78bfa'];

export function Confetti({ show }: ConfettiProps) {
  const [pieces, setPieces] = useState<Array<{ id: number; delay: number; color: string; x: number }>>([]);

  useEffect(() => {
    if (show) {
      const newPieces = Array.from({ length: 30 }, (_, i) => ({
        id: i,
        delay: Math.random() * 0.3,
        color: confettiColors[Math.floor(Math.random() * confettiColors.length)],
        x: 20 + Math.random() * 60,
      }));
      setPieces(newPieces);
    } else {
      setPieces([]);
    }
  }, [show]);

  return (
    <AnimatePresence>
      {show && pieces.length > 0 && (
        <div className="pointer-events-none fixed inset-x-0 top-0 z-50 h-32 overflow-visible">
          {pieces.map((piece) => (
            <ConfettiPiece key={piece.id} {...piece} />
          ))}
        </div>
      )}
    </AnimatePresence>
  );
}
