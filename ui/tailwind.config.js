/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // Terminal aesthetic (matches design mockups).
        void: '#0a0a0e',
        abyss: '#0e0e13',
        obsidian: '#121218',
        smoke: '#161621',
        ash: '#252530',
        silver: '#9a9aaa',
        pearl: '#e5e5e5',

        oracle: '#00d4ff',
        'oracle-dim': '#0095b2',
        'oracle-glow': 'rgba(0, 212, 255, 0.16)',
        'oracle-deep': '#004b5f',

        optimal: '#a3e635',
        'optimal-dim': '#6ea822',
        caution: '#fbbf24',
        'caution-dim': '#c98a3a',
        critical: '#ff4455',
        'critical-dim': '#c94444',
      },
      fontFamily: {
        sans: ["ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "Liberation Mono", "Courier New", "monospace"],
        display: ["ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "Liberation Mono", "Courier New", "monospace"],
        mono: ["ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "Liberation Mono", "Courier New", "monospace"],
      },
      animation: {
        'pulse-slow': 'pulse 4s cubic-bezier(0.4, 0, 0.6, 1) infinite',
        'float': 'float 8s ease-in-out infinite',
        'float-delayed': 'float 8s ease-in-out 2s infinite',
        'float-slow': 'float 12s ease-in-out infinite',
        'soft-slide': 'soft-slide 0.4s ease-out',
        'shimmer': 'shimmer 2.5s ease-in-out infinite',
        'glow-pulse': 'glow-pulse 3s ease-in-out infinite',
        'scan-line': 'scan-line 2s ease-in-out infinite',
        'gradient-shift': 'gradient-shift 6s ease infinite',
        'fade-in': 'fade-in 0.5s ease-out',
        'fade-in-up': 'fade-in-up 0.6s ease-out',
        'scale-in': 'scale-in 0.3s ease-out',
        'spin-slow': 'spin 3s linear infinite',
        'ring-expand': 'ring-expand 1s ease-out forwards',
        'border-flow': 'border-flow 4s linear infinite',
      },
      keyframes: {
        float: {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-10px)' },
        },
        'soft-slide': {
          '0%': { opacity: '0', transform: 'translateY(12px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        shimmer: {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        'glow-pulse': {
          '0%, 100%': { opacity: '0.4' },
          '50%': { opacity: '1' },
        },
        'scan-line': {
          '0%': { transform: 'translateY(-100%)' },
          '50%': { transform: 'translateY(100%)' },
          '100%': { transform: 'translateY(-100%)' },
        },
        'gradient-shift': {
          '0%, 100%': { backgroundPosition: '0% 50%' },
          '50%': { backgroundPosition: '100% 50%' },
        },
        'fade-in': {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        'fade-in-up': {
          '0%': { opacity: '0', transform: 'translateY(20px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'scale-in': {
          '0%': { opacity: '0', transform: 'scale(0.9)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
        'ring-expand': {
          '0%': { transform: 'scale(1)', opacity: '0.6' },
          '100%': { transform: 'scale(2.5)', opacity: '0' },
        },
        'border-flow': {
          '0%': { backgroundPosition: '0% 50%' },
          '100%': { backgroundPosition: '300% 50%' },
        },
      },
      boxShadow: {
        'oracle': '0 8px 32px rgba(25, 212, 255, 0.22)',
        'oracle-strong': '0 14px 48px rgba(25, 212, 255, 0.3)',
        'oracle-subtle': '0 4px 16px rgba(25, 212, 255, 0.12)',
        'optimal': '0 8px 32px rgba(121, 242, 166, 0.2)',
        'caution-glow': '0 8px 32px rgba(255, 180, 84, 0.18)',
        'critical-glow': '0 8px 32px rgba(255, 107, 107, 0.18)',
        'inner-glow': 'inset 0 1px 0 0 rgba(255,255,255,0.05)',
        'panel-elevated': '0 8px 40px rgba(0,0,0,0.4), 0 2px 8px rgba(0,0,0,0.2)',
      },
      backgroundImage: {
        'gradient-radial': 'radial-gradient(var(--tw-gradient-stops))',
        'gradient-oracle': 'linear-gradient(135deg, #19d4ff 0%, #0a6f8a 100%)',
        'gradient-oracle-text': 'linear-gradient(135deg, #19d4ff 0%, #79f2a6 100%)',
        'gradient-warm': 'linear-gradient(135deg, #ffb454 0%, #ff6b6b 100%)',
        'gradient-mesh': 'radial-gradient(ellipse 60% 40% at 50% -10%, rgba(25,212,255,0.08), transparent), radial-gradient(ellipse 40% 30% at 80% 60%, rgba(121,242,166,0.04), transparent), radial-gradient(ellipse 50% 35% at 10% 80%, rgba(255,180,84,0.03), transparent)',
      },
    },
  },
  plugins: [],
}
