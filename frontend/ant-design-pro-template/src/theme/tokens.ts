export const SPACING = {
  xs: 8,
  sm: 12,
  md: 16,
  lg: 24,
  xl: 32,
  xxl: 48,
} as const;

export const RADIUS = {
  sm: 6,
  md: 8,
  lg: 12,
  xl: 16,
} as const;

export const SHADOWS = {
  card: '0 1px 3px 0 rgba(0,0,0,0.06), 0 1px 2px -1px rgba(0,0,0,0.06)',
  cardHover:
    '0 10px 15px -3px rgba(0,0,0,0.08), 0 4px 6px -4px rgba(0,0,0,0.05)',
  elevated:
    '0 20px 25px -5px rgba(0,0,0,0.1), 0 8px 10px -6px rgba(0,0,0,0.05)',
} as const;

export const GRADIENTS = {
  primary: 'linear-gradient(135deg, #6366f1 0%, #8b5cf6 100%)',
  page: 'linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%)',
  cardPrimary: 'linear-gradient(135deg, #eef2ff 0%, #e0e7ff 100%)',
  cardSuccess: 'linear-gradient(135deg, #ecfdf5 0%, #d1fae5 100%)',
  cardWarning: 'linear-gradient(135deg, #fffbeb 0%, #fef3c7 100%)',
  cardInfo: 'linear-gradient(135deg, #ecfeff 0%, #cffafe 100%)',
} as const;

export const ACCENT_COLORS = {
  primary: '#6366f1',
  success: '#10b981',
  warning: '#f59e0b',
  error: '#ef4444',
  info: '#06b6d4',
  purple: '#8b5cf6',
} as const;

export const CHART_PALETTE = [
  '#6366f1',
  '#8b5cf6',
  '#06b6d4',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#ec4899',
  '#14b8a6',
  '#f97316',
  '#64748b',
];

export const CHART_DEFAULTS = {
  height: 320,
  pointSize: 3,
  lineWidth: 2.5,
  legendPosition: 'top-left' as const,
  animationDuration: 800,
  columnRadius: [6, 6, 0, 0] as [number, number, number, number],
};
